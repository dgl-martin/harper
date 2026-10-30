import type { DialectInfo } from 'harper.js';
import { describe, expect, test } from 'vitest';
import { createEditorLanguageOptions, resolveEditorLanguage } from './editor-language';

const catalog = [
	{ dialect: 0, language: 'English', region: 'American', code: 'US' },
	{ dialect: 1, language: 'English', region: 'British', code: 'GB' },
	{ dialect: 2, language: 'English', region: 'Australian', code: 'AU' },
	{ dialect: 3, language: 'English', region: 'Canadian', code: 'CA' },
	{ dialect: 4, language: 'English', region: 'Indian', code: 'IN' },
	{ dialect: 5, language: 'Deutsch', region: 'Deutschland', code: 'DE' },
	{ dialect: 6, language: 'Deutsch', region: 'Österreich', code: 'AT' },
	{ dialect: 7, language: 'Deutsch', region: 'Schweiz', code: 'CH' },
] as DialectInfo[];

describe('editor language selector', () => {
	test('maps English and German choices to compiled dialects', () => {
		const options = createEditorLanguageOptions(catalog);

		expect(options.map(({ value, dialect }) => ({ value, dialect }))).toEqual([
			{ value: 'en-US', dialect: 0 },
			{ value: 'en-GB', dialect: 1 },
			{ value: 'en-AU', dialect: 2 },
			{ value: 'en-CA', dialect: 3 },
			{ value: 'en-IN', dialect: 4 },
			{ value: 'de-DE', dialect: 5 },
			{ value: 'de-AT', dialect: 6 },
			{ value: 'de-CH', dialect: 7 },
		]);
	});

	test('uses the resolved desktop language as the initial editor language', () => {
		const options = createEditorLanguageOptions(catalog);

		expect(
			resolveEditorLanguage({ mode: 'explicit', language: 'de', region: 'AT' }, options),
		).toMatchObject({ value: 'de-AT', dialect: 6 });
	});

	test('reports a missing required dialect instead of silently using English', () => {
		expect(() => createEditorLanguageOptions(catalog.filter(({ code }) => code !== 'DE'))).toThrow(
			'The grammar engine does not include Deutsch (Deutschland).',
		);
	});
});
