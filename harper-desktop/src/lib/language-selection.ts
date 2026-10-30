export type LanguageSelection =
	| {
			mode: 'system';
			language: null;
			region: null;
	  }
	| {
			mode: 'explicit';
			language: string;
			region: string;
	  };

export const LANGUAGE_OPTIONS = [
	{ value: 'system', label: 'Automatic (System Language)' },
	{ value: 'en-US', label: 'English (United States)' },
	{ value: 'en-GB', label: 'English (United Kingdom)' },
	{ value: 'en-AU', label: 'English (Australia)' },
	{ value: 'en-CA', label: 'English (Canada)' },
	{ value: 'en-IN', label: 'English (India)' },
	{ value: 'de-DE', label: 'Deutsch (Deutschland)' },
	{ value: 'de-AT', label: 'Deutsch (Österreich)' },
	{ value: 'de-CH', label: 'Deutsch (Schweiz)' },
] as const;

export type LanguageSettingsValue = (typeof LANGUAGE_OPTIONS)[number]['value'];

export function parseLanguageSelection(value: unknown): LanguageSelection {
	if (typeof value !== 'object' || value == null) {
		throw new Error('Language selection response must be an object.');
	}

	const selection = value as Record<string, unknown>;

	if (selection.mode === 'system' && selection.language === null && selection.region === null) {
		return { mode: 'system', language: null, region: null };
	}

	if (
		selection.mode === 'explicit' &&
		typeof selection.language === 'string' &&
		typeof selection.region === 'string'
	) {
		return {
			mode: 'explicit',
			language: selection.language,
			region: selection.region,
		};
	}

	throw new Error('Language selection response has an invalid shape.');
}

export function languageSelectionToSettingsValue(
	selection: LanguageSelection,
): LanguageSettingsValue {
	if (selection.mode === 'system') {
		return 'system';
	}

	const value = `${selection.language}-${selection.region}`;
	const option = LANGUAGE_OPTIONS.find((candidate) => candidate.value === value);

	if (!option || option.value === 'system') {
		throw new Error(`Unsupported language selection: ${value}`);
	}

	return option.value;
}

export function settingsValueToLanguageSelection(value: string): LanguageSelection {
	if (value === 'system') {
		return { mode: 'system', language: null, region: null };
	}

	const option = LANGUAGE_OPTIONS.find((candidate) => candidate.value === value);

	if (!option || option.value === 'system') {
		throw new Error(`Unsupported language option: ${value}`);
	}

	const [language, region] = option.value.split('-');
	return { mode: 'explicit', language, region };
}
