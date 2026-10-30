import { describe, expect, test } from 'vitest';
import {
	LANGUAGE_OPTIONS,
	languageSelectionToSettingsValue,
	parseLanguageSelection,
	settingsValueToLanguageSelection,
} from './language-selection';

describe('desktop language selection', () => {
	test.each([
		['system', { mode: 'system', language: null, region: null }],
		['en-US', { mode: 'explicit', language: 'en', region: 'US' }],
		['en-GB', { mode: 'explicit', language: 'en', region: 'GB' }],
		['en-AU', { mode: 'explicit', language: 'en', region: 'AU' }],
		['en-CA', { mode: 'explicit', language: 'en', region: 'CA' }],
		['en-IN', { mode: 'explicit', language: 'en', region: 'IN' }],
		['de-DE', { mode: 'explicit', language: 'de', region: 'DE' }],
		['de-AT', { mode: 'explicit', language: 'de', region: 'AT' }],
		['de-CH', { mode: 'explicit', language: 'de', region: 'CH' }],
	] as const)('round-trips %s', (value, selection) => {
		expect(settingsValueToLanguageSelection(value)).toEqual(selection);
		expect(languageSelectionToSettingsValue(selection)).toBe(value);
	});

	test('exposes the required automatic, English, and German options', () => {
		expect(LANGUAGE_OPTIONS.map(({ value }) => value)).toEqual([
			'system',
			'en-US',
			'en-GB',
			'en-AU',
			'en-CA',
			'en-IN',
			'de-DE',
			'de-AT',
			'de-CH',
		]);
	});

	test('does not map an unknown backend language to American English', () => {
		expect(() =>
			languageSelectionToSettingsValue({
				mode: 'explicit',
				language: 'pt',
				region: 'BR',
			}),
		).toThrow('Unsupported language selection: pt-BR');
	});

	test('rejects malformed backend responses', () => {
		expect(() =>
			parseLanguageSelection({ mode: 'explicit', language: 'de', region: null }),
		).toThrow('Language selection response has an invalid shape.');
	});
});
