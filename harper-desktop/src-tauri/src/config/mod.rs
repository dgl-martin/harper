mod error;
mod integration;

use crate::{PlatformBroker, os_broker::OsBroker};
pub use error::Error;
pub use integration::Integration;

use harper_core::language::{new_curated_for_language, parse_language, parser_for_prose};
use harper_core::{
    Dialect, DictWordMetadata, Document, IgnoredLints, Language,
    linting::{FlatConfig, LintGroup},
    parsers::MarkdownOptions,
    spell::{Dictionary, MergedDictionary, MutableDictionary},
};
use harper_dictionary_wordlist::{load_dict, save_dict};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, Error as _},
};
use std::{fs, io, path::PathBuf, sync::Arc};

const LANGUAGE_VARIANTS: &[(&str, &str, &str)] = &[
    ("en", "US", "en-US"),
    ("en", "GB", "en-GB"),
    ("en", "AU", "en-AU"),
    ("en", "CA", "en-CA"),
    ("en", "IN", "en-IN"),
    ("de", "DE", "de-DE"),
    ("de", "AT", "de-AT"),
    ("de", "CH", "de-CH"),
    ("pl", "PL", "pl-PL"),
    ("pt", "PT", "pt-PT"),
    ("pt", "BR", "pt-BR"),
    ("pt", "AO", "ao"),
    ("sk", "SK", "sk-SK"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSelection {
    pub mode: LanguageSelectionMode,
    pub language: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageSelectionMode {
    System,
    Explicit,
}

impl LanguageSelection {
    pub fn system() -> Self {
        Self {
            mode: LanguageSelectionMode::System,
            language: None,
            region: None,
        }
    }

    pub fn explicit(language: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            mode: LanguageSelectionMode::Explicit,
            language: Some(language.into()),
            region: Some(region.into()),
        }
    }

    pub fn from_language(language: Language) -> Result<Self, String> {
        LANGUAGE_VARIANTS
            .iter()
            .find(|(_, _, alias)| parse_language(alias) == Some(language))
            .map(|(language, region, _)| Self::explicit(*language, *region))
            .ok_or_else(|| format!("unsupported desktop language: {language}"))
    }

    fn normalize(self) -> Result<(Self, Language), String> {
        let locale = tauri_plugin_os::locale();
        self.normalize_with_locale(locale.as_deref())
    }

    fn normalize_with_locale(
        self,
        system_locale: Option<&str>,
    ) -> Result<(Self, Language), String> {
        match self.mode {
            LanguageSelectionMode::System => {
                if self.language.is_some() || self.region.is_some() {
                    return Err(
                        "system language selection must not include language or region".to_string(),
                    );
                }

                Ok((
                    Self::system(),
                    system_locale.and_then(parse_language).unwrap_or_default(),
                ))
            }
            LanguageSelectionMode::Explicit => {
                let language = self
                    .language
                    .as_deref()
                    .ok_or_else(|| "explicit language selection requires `language`".to_string())?;
                let region = self
                    .region
                    .as_deref()
                    .ok_or_else(|| "explicit language selection requires `region`".to_string())?;
                let (language, region, alias) = LANGUAGE_VARIANTS
                    .iter()
                    .find(|(candidate_language, candidate_region, _)| {
                        candidate_language.eq_ignore_ascii_case(language)
                            && candidate_region.eq_ignore_ascii_case(region)
                    })
                    .ok_or_else(|| {
                        format!("unsupported desktop language selection: {language}-{region}")
                    })?;
                let dialect = parse_language(alias).ok_or_else(|| {
                    format!("language support is not enabled for {language}-{region}")
                })?;

                Ok((Self::explicit(*language, *region), dialect))
            }
        }
    }
}

/// User-controlled app state needed by Tauri commands and the highlighter process.
pub struct Config {
    pub mutable_dictionary: MutableDictionary,
    pub language_selection: LanguageSelection,
    pub dialect: Language,
    pub ignored_lints: IgnoredLints,
    pub lint_config: FlatConfig,
    pub integrations: Vec<Integration>,
    pub auto_enable_new_apps: bool,
    pub onboarding_completed: bool,
    pub debounce_ms: u64,
    /// Whether the main process may check for and install updates automatically.
    pub auto_update: bool,
    /// Unix milliseconds at the start of the last manual or automatic update attempt.
    /// Failed attempts also count toward the automatic 24-hour interval.
    pub last_update_check: Option<u64>,
    pub highlighter_service_enabled: bool,
}

impl Config {
    pub fn new() -> Self {
        let language_selection = LanguageSelection::system();

        Self {
            mutable_dictionary: MutableDictionary::new(),
            dialect: Self::detect_system_dialect(),
            language_selection,
            ignored_lints: IgnoredLints::new(),
            lint_config: FlatConfig::new_curated(),
            integrations: Integration::curated_integrations(),
            auto_enable_new_apps: false,
            onboarding_completed: false,
            debounce_ms: 0,
            auto_update: true,
            last_update_check: None,
            highlighter_service_enabled: true,
        }
    }

    fn detect_system_dialect() -> Language {
        tauri_plugin_os::locale()
            .and_then(|bcp47| parse_language(&bcp47))
            .unwrap_or_default()
    }

    pub fn set_language_selection(
        &mut self,
        language_selection: LanguageSelection,
    ) -> Result<(), String> {
        let (language_selection, dialect) = language_selection.normalize()?;
        let words = self
            .mutable_dictionary
            .words_iter()
            .map(|word| word.iter().collect::<String>())
            .collect::<Vec<_>>();
        self.language_selection = language_selection;
        self.dialect = dialect;
        self.mutable_dictionary = Self::mutable_dictionary_from_words(words, dialect);

        Ok(())
    }

    pub fn mutable_dictionary_from_words(
        words: impl IntoIterator<Item = String>,
        language: Language,
    ) -> MutableDictionary {
        let mut dictionary = MutableDictionary::new();
        dictionary.extend_words(words.into_iter().map(|word| {
            (
                word.chars().collect::<Vec<_>>(),
                DictWordMetadata {
                    dialects: language.dictionary_dialect_flags(),
                    ..Default::default()
                },
            )
        }));

        dictionary
    }

    pub fn is_integration_enabled(&self, bundle_id: &str) -> bool {
        let integrations: &[Integration] = &self.integrations;
        integrations
            .iter()
            .any(|integration| integration.bundle_id == bundle_id && integration.enabled)
    }

    /// Resolves an app's enabled state, registering unknown apps when automatic enablement is on.
    /// Existing disabled entries are never re-enabled; removed entries can be discovered again.
    /// Harper itself is excluded from discovery, but explicitly configured entries are respected.
    /// The caller is responsible for persisting any newly registered integration.
    pub fn resolve_integration(&mut self, bundle_id: &str) -> bool {
        let bundle_id = bundle_id.trim();
        if bundle_id.is_empty() {
            return false;
        }
        if let Some(integration) = self
            .integrations
            .iter()
            .find(|item| item.bundle_id == bundle_id)
        {
            return integration.enabled;
        }
        if !self.auto_enable_new_apps || PlatformBroker::is_harper_desktop(bundle_id) {
            return false;
        }
        self.add_integration(bundle_id.to_owned());
        true
    }

    pub fn add_integration(&mut self, bundle_id: String) {
        let bundle_id = bundle_id.trim();
        if bundle_id.is_empty()
            || self
                .integrations
                .iter()
                .any(|item| item.bundle_id == bundle_id)
        {
            return;
        }

        self.integrations.push(Integration {
            bundle_id: bundle_id.to_string(),
            enabled: true,
        });
        self.integrations
            .sort_by(|a, b| a.bundle_id.cmp(&b.bundle_id));
    }

    pub fn remove_integration(&mut self, bundle_id: &str) {
        self.integrations
            .retain(|integration| integration.bundle_id != bundle_id);
    }

    pub fn set_integration_enabled(&mut self, bundle_id: &str, enabled: bool) {
        if let Some(integration) = self
            .integrations
            .iter_mut()
            .find(|integration| integration.bundle_id == bundle_id)
        {
            integration.enabled = enabled;
        }
    }

    pub async fn save_to_system(&self) -> Result<(), Error> {
        let folder_path = Self::folder_path().ok_or(Error::ConfigDirUnavailable)?;
        let main_path = Self::main_path().ok_or(Error::ConfigDirUnavailable)?;
        let dictionary_path = Self::dictionary_path().ok_or(Error::ConfigDirUnavailable)?;

        fs::create_dir_all(folder_path)?;
        fs::write(main_path, self.serialize_main()?)?;
        save_dict(dictionary_path, &self.mutable_dictionary).await?;

        Ok(())
    }

    pub fn main_config_exists() -> Result<bool, Error> {
        let main_path = Self::main_path().ok_or(Error::ConfigDirUnavailable)?;

        match fs::metadata(main_path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn load_from_system() -> Result<Self, Error> {
        let main_path = Self::main_path().ok_or(Error::ConfigDirUnavailable)?;
        let dictionary_path = Self::dictionary_path().ok_or(Error::ConfigDirUnavailable)?;
        let serialized = fs::read_to_string(main_path)?;
        let mut config = Self::deserialize_main(&serialized)?;
        config.lint_config.fill_with_curated();
        config.mutable_dictionary =
            load_dict(dictionary_path, config.dialect.dictionary_dialect_flags()).await?;

        Ok(config)
    }

    pub fn dictionary_from_user_dictionary(
        language: Language,
        user_dictionary: MutableDictionary,
    ) -> Arc<MergedDictionary> {
        let mut dictionary = MergedDictionary::new();
        dictionary.add_dictionary(harper_core::language::dictionary(language));
        dictionary.add_dictionary(Arc::new(user_dictionary));

        Arc::new(dictionary)
    }

    pub fn document_from_user_dictionary(
        text: &str,
        language: Language,
        user_dictionary: MutableDictionary,
    ) -> Document {
        let dictionary = Self::dictionary_from_user_dictionary(language, user_dictionary);
        let parser = parser_for_prose("markdown", language, MarkdownOptions::default())
            .expect("markdown parser must exist for every desktop language");

        Document::new(text, &parser, &dictionary)
    }

    fn create_dictionary(&self) -> Arc<MergedDictionary> {
        Self::dictionary_from_user_dictionary(self.dialect, self.mutable_dictionary.clone())
    }

    #[cfg(test)]
    fn create_document(&self, text: &str) -> Document {
        Self::document_from_user_dictionary(text, self.dialect, self.mutable_dictionary.clone())
    }

    pub fn create_linter(&self) -> LintGroup {
        let linter = new_curated_for_language(self.create_dictionary(), self.dialect);
        let mut lint_config = linter.config.clone();
        lint_config.merge_from(self.lint_config.clone());

        linter.with_lint_config(lint_config)
    }

    #[allow(dead_code)]
    fn folder_path() -> Option<PathBuf> {
        dirs::config_dir().map(|path| path.join("harper-desktop"))
    }

    #[allow(dead_code)]
    fn main_path() -> Option<PathBuf> {
        Self::folder_path().map(|path| path.join("config.json"))
    }

    #[allow(dead_code)]
    fn dictionary_path() -> Option<PathBuf> {
        Self::folder_path().map(|path| path.join("dictionary.txt"))
    }

    #[allow(dead_code)]
    fn serialize_main(&self) -> serde_json::Result<String> {
        serde_json::to_string(&serde_json::json!({
            "dialect": serialize_language_compat(self.dialect),
            "language_selection": &self.language_selection,
            "ignored_lints": &self.ignored_lints,
            "lint_config": &self.lint_config,
            "integrations": &self.integrations,
            "auto_enable_new_apps": self.auto_enable_new_apps,
            "onboarding_completed": self.onboarding_completed,
            "debounce_ms": self.debounce_ms,
            "auto_update": self.auto_update,
            "last_update_check": self.last_update_check,
            "highlighter_service_enabled": self.highlighter_service_enabled,
        }))
    }

    #[allow(dead_code)]
    fn deserialize_main(serialized: &str) -> serde_json::Result<Self> {
        let mut value = serde_json::from_str::<serde_json::Value>(serialized)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| serde_json::Error::custom("config must be a JSON object"))?;

        let legacy_dialect = {
            let value = object
                .remove("dialect")
                .ok_or_else(|| serde_json::Error::custom("missing config field `dialect`"))?;
            deserialize_language_compat(value)?
        };
        let (language_selection, dialect) =
            match deserialize_optional_field::<LanguageSelection>(object, "language_selection")? {
                Some(language_selection) => language_selection
                    .normalize()
                    .map_err(serde_json::Error::custom)?,
                None => (
                    LanguageSelection::from_language(legacy_dialect)
                        .map_err(serde_json::Error::custom)?,
                    legacy_dialect,
                ),
            };

        Ok(Self {
            mutable_dictionary: MutableDictionary::new(),
            language_selection,
            dialect,
            ignored_lints: deserialize_field(object, "ignored_lints")?,
            lint_config: deserialize_field(object, "lint_config")?,
            integrations: deserialize_optional_field(object, "integrations")?
                .unwrap_or_else(Integration::curated_integrations),
            auto_enable_new_apps: deserialize_optional_field(object, "auto_enable_new_apps")?
                .unwrap_or(false),
            onboarding_completed: deserialize_optional_field(object, "onboarding_completed")?
                .unwrap_or(true),
            debounce_ms: deserialize_optional_field(object, "debounce_ms")?.unwrap_or(0),
            auto_update: deserialize_optional_field(object, "auto_update")?.unwrap_or(true),
            last_update_check: deserialize_optional_field::<Option<u64>>(
                object,
                "last_update_check",
            )?
            .flatten(),
            highlighter_service_enabled: deserialize_optional_field(
                object,
                "highlighter_service_enabled",
            )?
            .unwrap_or(true),
        })
    }
}

#[allow(dead_code)]
fn deserialize_field<T>(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> serde_json::Result<T>
where
    T: DeserializeOwned,
{
    let value = object
        .remove(field)
        .ok_or_else(|| serde_json::Error::custom(format!("missing config field `{field}`")))?;

    serde_json::from_value(value)
}

#[allow(dead_code)]
fn deserialize_optional_field<T>(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> serde_json::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let Some(value) = object.remove(field) else {
        return Ok(None);
    };

    serde_json::from_value(value).map(Some)
}

/// Serialize the `dialect` config field. English is written as a plain dialect
/// string, which is what released builds read; the other languages use the
/// externally-tagged `Language` form.
///
/// Writing the released format for English keeps the file loadable by a build
/// that predates `Language`. Such a build parses `dialect` as a bare `Dialect`
/// and fails the whole document if it cannot, which silently resets the
/// ignored lints, rule config and integrations along with it.
fn serialize_language_compat(language: Language) -> serde_json::Value {
    match language {
        Language::English(dialect) => serde_json::json!(dialect),
        other => serde_json::json!(other),
    }
}

/// Deserialize the `dialect` config field, accepting both the current
/// externally-tagged `Language` format and a plain `Dialect` string such as
/// `"American"`, which is what the setting held before it became a `Language`.
fn deserialize_language_compat(value: serde_json::Value) -> serde_json::Result<Language> {
    if let Ok(language) = serde_json::from_value::<Language>(value.clone()) {
        return Ok(language);
    }

    if value.is_string()
        && let Ok(dialect) = serde_json::from_value::<Dialect>(value)
    {
        return Ok(Language::English(dialect));
    }

    Err(serde_json::Error::custom(
        "invalid `dialect` field: expected a Language object like \
         {\"English\": \"American\"} or a plain dialect string like \"American\"",
    ))
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, Integration, LanguageSelection, LanguageSelectionMode};
    use harper_core::{Dialect, DictWordMetadata, Language, linting::Linter};

    #[test]
    fn serialize_main_excludes_dictionary_word_list() {
        let mut config = Config::new();
        config
            .mutable_dictionary
            .append_word_str("blorple", DictWordMetadata::default());

        let serialized = config.serialize_main().unwrap();

        assert!(!serialized.contains("mutable_dictionary"));
        assert!(!serialized.contains("blorple"));
        assert!(serialized.contains("dialect"));
        assert!(serialized.contains("language_selection"));
        assert!(serialized.contains("ignored_lints"));
        assert!(serialized.contains("lint_config"));
        assert!(serialized.contains("integrations"));
        assert!(serialized.contains("onboarding_completed"));
        assert!(serialized.contains("debounce_ms"));
        assert!(serialized.contains("auto_update"));
        assert!(serialized.contains("last_update_check"));
        assert!(serialized.contains("highlighter_service_enabled"));
    }

    #[test]
    fn serialize_main_writes_english_as_a_plain_dialect_string() {
        let mut config = Config::new();
        config
            .set_language_selection(LanguageSelection::explicit("en", "GB"))
            .unwrap();

        let serialized = config.serialize_main().unwrap();

        // The format released builds read. Writing the tagged form here makes
        // them fail the whole document and reset every other setting.
        assert!(
            serialized.contains(r#""dialect":"British""#),
            "{serialized}"
        );

        assert_eq!(
            Config::deserialize_main(&serialized).unwrap().dialect,
            Language::English(Dialect::British)
        );
    }

    #[cfg(feature = "de")]
    #[test]
    fn serialize_main_round_trips_a_non_english_language() {
        use harper_core::language::german::dialects::GermanDialect;

        let mut config = Config::new();
        config
            .set_language_selection(LanguageSelection::explicit("de", "DE"))
            .unwrap();

        let serialized = config.serialize_main().unwrap();

        assert!(
            serialized.contains(r#""dialect":{"German":"#),
            "{serialized}"
        );
        assert_eq!(
            Config::deserialize_main(&serialized).unwrap().dialect,
            Language::German(GermanDialect::Standard)
        );
    }

    #[test]
    fn deserialize_main_restores_main_serialized_fields() {
        let mut config = Config::new();
        config
            .mutable_dictionary
            .append_word_str("blorple", DictWordMetadata::default());
        let serialized = config.serialize_main().unwrap();

        let deserialized = Config::deserialize_main(&serialized).unwrap();

        assert_eq!(deserialized.dialect, config.dialect);
        assert_eq!(deserialized.lint_config, config.lint_config);
        assert_eq!(deserialized.integrations, config.integrations);
        assert_eq!(
            deserialized.onboarding_completed,
            config.onboarding_completed
        );
        assert_eq!(deserialized.debounce_ms, config.debounce_ms);
        assert_eq!(deserialized.auto_update, config.auto_update);
        assert_eq!(deserialized.last_update_check, config.last_update_check);
        assert_eq!(
            deserialized.highlighter_service_enabled,
            config.highlighter_service_enabled
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&deserialized.serialize_main().unwrap())
                .unwrap(),
            serde_json::from_str::<serde_json::Value>(&serialized).unwrap()
        );
    }

    #[test]
    fn deserialize_main_accepts_plain_dialect_string() {
        let config = Config::new();
        let mut value =
            serde_json::from_str::<serde_json::Value>(&config.serialize_main().unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "dialect".to_string(),
            serde_json::Value::String("American".to_string()),
        );
        value.as_object_mut().unwrap().remove("language_selection");

        let deserialized = Config::deserialize_main(&value.to_string()).unwrap();

        assert_eq!(deserialized.dialect, Language::English(Dialect::American));
        assert_eq!(
            deserialized.language_selection,
            LanguageSelection::explicit("en", "US")
        );
    }

    #[test]
    fn deserialize_main_accepts_new_language_dialect() {
        let config = Config::new();
        let mut value =
            serde_json::from_str::<serde_json::Value>(&config.serialize_main().unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "dialect".to_string(),
            serde_json::json!({ "English": "British" }),
        );
        value.as_object_mut().unwrap().remove("language_selection");

        let deserialized = Config::deserialize_main(&value.to_string()).unwrap();

        assert_eq!(deserialized.dialect, Language::English(Dialect::British));
        assert_eq!(
            deserialized.language_selection,
            LanguageSelection::explicit("en", "GB")
        );
    }

    #[test]
    fn system_language_selection_resolves_supported_german_locales() {
        for (locale, expected_region) in [("de-DE", "DE"), ("de-AT", "AT"), ("de-CH", "CH")] {
            let (_, dialect) = LanguageSelection::system()
                .normalize_with_locale(Some(locale))
                .unwrap();
            let expected = LanguageSelection::explicit("de", expected_region)
                .normalize_with_locale(None)
                .unwrap()
                .1;

            assert_eq!(dialect, expected);
        }
    }

    #[test]
    fn explicit_language_selection_supports_english_and_german_variants() {
        for (language, region) in [
            ("en", "US"),
            ("en", "GB"),
            ("en", "AU"),
            ("en", "CA"),
            ("en", "IN"),
            ("de", "DE"),
            ("de", "AT"),
            ("de", "CH"),
        ] {
            let (selection, _) = LanguageSelection::explicit(language, region)
                .normalize_with_locale(None)
                .unwrap();

            assert_eq!(
                selection,
                LanguageSelection::explicit(language, region),
                "{language}-{region}"
            );
        }
    }

    #[test]
    fn changing_language_retags_user_dictionary_words() {
        use harper_core::spell::Dictionary;

        let mut config = Config::new();
        config
            .mutable_dictionary
            .append_word_str("Harperwort", DictWordMetadata::default());
        config
            .set_language_selection(LanguageSelection::explicit("de", "AT"))
            .unwrap();

        let metadata = config
            .mutable_dictionary
            .get_word_metadata_str("Harperwort")
            .unwrap();
        assert_eq!(metadata.dialects, config.dialect.dictionary_dialect_flags());
    }

    #[test]
    fn invalid_language_selections_are_rejected() {
        let invalid_system = LanguageSelection {
            mode: LanguageSelectionMode::System,
            language: Some("de".to_string()),
            region: Some("DE".to_string()),
        };

        assert!(invalid_system.normalize_with_locale(Some("de-DE")).is_err());
        assert!(
            LanguageSelection::explicit("de", "US")
                .normalize_with_locale(None)
                .is_err()
        );
    }

    #[test]
    fn deserialize_main_uses_zero_debounce_when_missing() {
        let config = Config::new();
        let mut value =
            serde_json::from_str::<serde_json::Value>(&config.serialize_main().unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("debounce_ms");

        let deserialized = Config::deserialize_main(&value.to_string()).unwrap();

        assert_eq!(deserialized.debounce_ms, 0);
    }

    #[test]
    fn deserialize_main_uses_default_update_settings_when_missing() {
        let config = Config::new();
        let mut value =
            serde_json::from_str::<serde_json::Value>(&config.serialize_main().unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("auto_update");
        value.as_object_mut().unwrap().remove("last_update_check");

        let deserialized = Config::deserialize_main(&value.to_string()).unwrap();

        assert!(deserialized.auto_update);
        assert_eq!(deserialized.last_update_check, None);
    }

    #[test]
    fn deserialize_main_enables_highlighter_service_when_missing() {
        let config = Config::new();
        let mut value =
            serde_json::from_str::<serde_json::Value>(&config.serialize_main().unwrap()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("highlighter_service_enabled");

        let deserialized = Config::deserialize_main(&value.to_string()).unwrap();

        assert!(deserialized.highlighter_service_enabled);
    }

    #[test]
    fn deserialize_main_preserves_integrations() {
        let mut config = Config::new();
        config.integrations = vec![Integration {
            bundle_id: "com.example.Editor".to_string(),
            enabled: false,
        }];

        let deserialized = Config::deserialize_main(&config.serialize_main().unwrap()).unwrap();

        assert_eq!(
            deserialized.integrations,
            vec![Integration {
                bundle_id: "com.example.Editor".to_string(),
                enabled: false,
            }]
        );
    }

    #[test]
    fn integration_helpers_add_remove_enable_and_check() {
        let mut config = Config::new();

        config.add_integration(" com.example.Editor ".to_string());
        config.add_integration("com.example.Editor".to_string());
        assert!(config.is_integration_enabled("com.example.Editor"));
        assert_eq!(
            config
                .integrations
                .iter()
                .filter(|integration| integration.bundle_id == "com.example.Editor")
                .count(),
            1
        );

        config.set_integration_enabled("com.example.Editor", false);
        assert!(!config.is_integration_enabled("com.example.Editor"));

        config.remove_integration("com.example.Editor");

        assert!(
            !config
                .integrations
                .iter()
                .any(|integration| integration.bundle_id == "com.example.Editor")
        );
    }

    #[test]
    fn main_path_points_to_harper_desktop_config_file() {
        let path = Config::main_path().unwrap();

        assert_eq!(path.file_name().unwrap(), "config.json");
        assert_eq!(
            path.parent().unwrap().file_name().unwrap(),
            "harper-desktop"
        );
    }

    #[cfg(feature = "de")]
    #[test]
    fn dictionary_for_german_contains_german_words() {
        use harper_core::Language;
        use harper_core::language::german::dialects::GermanDialect;
        use harper_core::spell::{Dictionary, MutableDictionary};
        let dict = Config::dictionary_from_user_dictionary(
            Language::German(GermanDialect::Standard),
            MutableDictionary::new(),
        );
        assert!(dict.contains_word("Haus".chars().collect::<Vec<_>>().as_slice()));
        assert!(dict.contains_word("Freiheit".chars().collect::<Vec<_>>().as_slice()));
    }

    #[cfg(feature = "de")]
    #[test]
    fn german_selection_drives_desktop_linting() {
        let mut config = Config::new();
        config
            .set_language_selection(LanguageSelection::explicit("de", "DE"))
            .unwrap();
        let correct = config.create_document("Der Hund ist im Garten. Die Katze schläft.");
        let incorrect =
            config.create_document("Der Hund ist da. dieser Satz ist klein. Worrt und flasch.");
        let correct_lints = config.create_linter().lint(&correct);
        let incorrect_lints = config.create_linter().lint(&incorrect);

        assert!(
            correct_lints.len() <= 2,
            "correct German text produced unexpected lints: {correct_lints:?}"
        );
        assert!(
            incorrect_lints.len() > correct_lints.len(),
            "intentional German errors were not detected: {incorrect_lints:?}"
        );
    }

    #[test]
    fn main_config_exists_reports_missing_file() {
        let exists = Config::main_config_exists().unwrap();
        let expected = Config::main_path().unwrap().exists();

        assert_eq!(exists, expected);
    }

    #[test]
    fn dictionary_path_points_to_harper_desktop_dictionary_file() {
        let path = Config::dictionary_path().unwrap();

        assert_eq!(path.file_name().unwrap(), "dictionary.txt");
        assert_eq!(
            path.parent().unwrap().file_name().unwrap(),
            "harper-desktop"
        );
    }
}
