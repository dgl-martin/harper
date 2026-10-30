<script lang="ts">
import { Button, Checkbox, CheckIcon, Input, Select, SettingRow } from 'components';
import { onMount } from 'svelte';
import { Client } from '$lib/client';
import {
	LANGUAGE_OPTIONS,
	languageSelectionToSettingsValue,
	settingsValueToLanguageSelection,
} from '$lib/language-selection';

let menuBar = true;
let launchAtStartup = false;
let autoUpdate = true;
let language = 'system';
let isLanguageLoading = true;
let isLanguageSaving = false;
let languageError = '';
let isLaunchAtStartupLoading = true;
let isLaunchAtStartupSaving = false;
let launchAtStartupError = '';
let isAutoUpdateLoading = true;
let isAutoUpdateSaving = false;
let isCheckingForUpdates = false;
let autoUpdateError = '';
let updateStatus = '';
let currentVersion = '';
let latestVersion = '';
let debounceMs = 0;
let debounceMsInput = '0';
let isDebounceLoading = true;
let isDebounceSaving = false;
let debounceError = '';

onMount(() => {
	void loadLanguage();
	void loadLaunchAtStartup();
	void loadAutoUpdate();
	void loadUpdateVersions();
	void loadDebounceMs();

	const refreshSettings = () => {
		if (!isLanguageSaving) {
			void loadLanguage();
		}

		if (!isLaunchAtStartupSaving) {
			void loadLaunchAtStartup();
		}

		if (!isAutoUpdateSaving) {
			void loadAutoUpdate();
		}

		if (!isCheckingForUpdates) {
			void loadUpdateVersions();
		}

		if (!isDebounceSaving) {
			void loadDebounceMs();
		}
	};

	window.addEventListener('focus', refreshSettings);

	return () => {
		window.removeEventListener('focus', refreshSettings);
	};
});

async function loadLanguage() {
	isLanguageLoading = true;
	languageError = '';

	try {
		language = languageSelectionToSettingsValue(await Client.getLanguageSelection());
	} catch (error) {
		languageError = `Unable to load language: ${error}`;
	} finally {
		isLanguageLoading = false;
	}
}

async function setLanguage(value: string) {
	isLanguageSaving = true;
	languageError = '';

	try {
		await Client.setLanguageSelection(settingsValueToLanguageSelection(value));
		language = value;
	} catch (error) {
		languageError = `Unable to save language: ${error}`;
	} finally {
		isLanguageSaving = false;
	}
}

async function loadLaunchAtStartup() {
	isLaunchAtStartupLoading = true;
	launchAtStartupError = '';

	try {
		launchAtStartup = await Client.getLaunchAtStartup();
	} catch (error) {
		launchAtStartupError = `Unable to load startup setting: ${error}`;
	} finally {
		isLaunchAtStartupLoading = false;
	}
}

async function setLaunchAtStartup(enabled: boolean) {
	const previousLaunchAtStartup = launchAtStartup;

	launchAtStartup = enabled;
	isLaunchAtStartupSaving = true;
	launchAtStartupError = '';

	try {
		await Client.setLaunchAtStartup(enabled);
	} catch (error) {
		launchAtStartup = previousLaunchAtStartup;
		launchAtStartupError = `Unable to save startup setting: ${error}`;
	} finally {
		isLaunchAtStartupSaving = false;
	}
}

async function loadAutoUpdate() {
	isAutoUpdateLoading = true;
	autoUpdateError = '';

	try {
		autoUpdate = await Client.getAutoUpdate();
	} catch (error) {
		autoUpdateError = `Unable to load update setting: ${error}`;
	} finally {
		isAutoUpdateLoading = false;
	}
}

async function setAutoUpdate(enabled: boolean) {
	const previousAutoUpdate = autoUpdate;

	autoUpdate = enabled;
	isAutoUpdateSaving = true;
	autoUpdateError = '';
	updateStatus = '';

	try {
		await Client.setAutoUpdate(enabled);
	} catch (error) {
		autoUpdate = previousAutoUpdate;
		autoUpdateError = `Unable to save update setting: ${error}`;
	} finally {
		isAutoUpdateSaving = false;
	}
}

async function loadUpdateVersions() {
	try {
		const [current, latest] = await Promise.all([
			Client.getCurrentVersion(),
			Client.getLatestVersion(),
		]);
		currentVersion = current;
		latestVersion = latest;
	} catch (error) {
		console.error('Unable to load update versions.', error);
	}
}

async function checkForUpdates() {
	isCheckingForUpdates = true;
	autoUpdateError = '';
	updateStatus = 'Checking for updates...';

	try {
		const result = await Client.updateToLatest();
		updateStatus = result.message;

		if (result.latestVersion != null) {
			latestVersion = result.latestVersion;
		}

		currentVersion = result.currentVersion ?? (await Client.getCurrentVersion());
	} catch (error) {
		autoUpdateError = `Unable to check for updates: ${error}`;
		updateStatus = '';
	} finally {
		isCheckingForUpdates = false;
	}
}

async function loadDebounceMs() {
	isDebounceLoading = true;
	debounceError = '';

	try {
		debounceMs = await Client.getDebounceMs();
		debounceMsInput = String(debounceMs);
	} catch (error) {
		debounceError = `Unable to load debounce delay: ${error}`;
	} finally {
		isDebounceLoading = false;
	}
}

async function saveDebounceMs() {
	const parsedDebounceMs = Number(debounceMsInput);

	if (!Number.isInteger(parsedDebounceMs) || parsedDebounceMs < 0) {
		debounceError = 'Debounce delay must be a non-negative whole number.';
		debounceMsInput = String(debounceMs);
		return;
	}

	const previousDebounceMs = debounceMs;
	debounceMs = parsedDebounceMs;
	debounceMsInput = String(parsedDebounceMs);
	isDebounceSaving = true;
	debounceError = '';

	try {
		await Client.setDebounceMs(parsedDebounceMs);
	} catch (error) {
		debounceMs = previousDebounceMs;
		debounceMsInput = String(previousDebounceMs);
		debounceError = `Unable to save debounce delay: ${error}`;
	} finally {
		isDebounceSaving = false;
	}
}
</script>

<section>
        <div class="stanza">
          <div class="eyebrow">General</div>
          <div class="rows">
            <SettingRow top>
              <strong>Keep Harper in the menu bar</strong>
              <p>Shows the Harper icon so you can open settings without opening the main app.</p>
              <Checkbox
                slot="control"
                appearance="settings"
                checked={menuBar}
                disabled
                title="Not wired yet"
              >
                {#if menuBar}<CheckIcon className="control-icon" />{/if}
              </Checkbox>
            </SettingRow>

            <SettingRow>
              <strong>Launch Harper at startup</strong>
              <p>Harper will start silently when you log in.</p>
              <Checkbox
                slot="control"
                appearance="settings"
                checked={launchAtStartup}
                disabled={isLaunchAtStartupLoading || isLaunchAtStartupSaving}
                on:click={() => setLaunchAtStartup(!launchAtStartup)}
              >
                {#if launchAtStartup}<CheckIcon className="control-icon" />{/if}
              </Checkbox>
            </SettingRow>
            {#if isLaunchAtStartupLoading}
              <p class="result-summary">Loading startup setting...</p>
            {:else if launchAtStartupError}
              <p class="result-summary">{launchAtStartupError}</p>
            {:else if isLaunchAtStartupSaving}
              <p class="result-summary">Saving startup setting...</p>
            {/if}

            <SettingRow top>
              <strong>Automatically check for updates</strong>
              <p>Updates are disabled for this German preview build.</p>
              <p class="result-summary">
                Current version: {currentVersion || 'loading...'} · Latest version: {latestVersion || 'loading...'}
              </p>
              <Checkbox
                slot="control"
                appearance="settings"
                checked={autoUpdate}
                disabled
                on:click={() => setAutoUpdate(!autoUpdate)}
              >
                {#if autoUpdate}<CheckIcon className="control-icon" />{/if}
              </Checkbox>
            </SettingRow>
            <div class="inline-row">
              <Button
                unstyled
                class="button"
                type="button"
                disabled
                on:click={checkForUpdates}
              >
                Updates Disabled
              </Button>
            </div>
            {#if isAutoUpdateLoading}
              <p class="result-summary">Loading update setting...</p>
            {:else if autoUpdateError}
              <p class="result-summary">{autoUpdateError}</p>
            {:else if isAutoUpdateSaving}
              <p class="result-summary">Saving update setting...</p>
            {:else if updateStatus}
              <p class="result-summary">{updateStatus}</p>
            {/if}
          </div>
        </div>

        <div class="divider"></div>

        <div class="stanza">
          <div class="eyebrow">Language</div>
          <p class="section-copy">
            Choose the language Harper uses to check spelling and grammar.
          </p>
          <div class="inline-row">
            <label for="language">Language:</label>
            <Select
              unstyled
              id="language"
              class="select wide"
              disabled={isLanguageLoading || isLanguageSaving}
              value={language}
              on:change={(event) => setLanguage((event.detail.currentTarget as HTMLSelectElement).value)}
            >
              {#each LANGUAGE_OPTIONS as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </Select>
          </div>
          {#if isLanguageLoading}
            <p class="result-summary">Loading language...</p>
          {:else if languageError}
            <p class="result-summary">{languageError}</p>
          {:else if isLanguageSaving}
            <p class="result-summary">Saving language...</p>
          {/if}
        </div>

        <div class="divider"></div>

        <div class="stanza">
          <div class="eyebrow">Writing</div>
          <p class="section-copy">
            Choose how long Harper waits after text changes before checking it. Use 0 ms for
            immediate checking.
          </p>
          <div class="inline-row">
            <label for="debounce-ms">Debounce delay:</label>
            <Input
              unstyled
              id="debounce-ms"
              class="select"
              type="number"
              min="0"
              step="50"
              disabled={isDebounceLoading || isDebounceSaving}
              bind:value={debounceMsInput}
              on:change={saveDebounceMs}
            />
            <span>ms</span>
          </div>
          {#if isDebounceLoading}
            <p class="result-summary">Loading debounce delay...</p>
          {:else if debounceError}
            <p class="result-summary">{debounceError}</p>
          {:else if isDebounceSaving}
            <p class="result-summary">Saving debounce delay...</p>
          {/if}
        </div>
      </section>
