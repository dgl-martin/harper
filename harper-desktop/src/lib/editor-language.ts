import type { Dialect, DialectInfo } from 'harper.js';
import {
	LANGUAGE_OPTIONS,
	type LanguageSelection,
	languageSelectionToSettingsValue,
} from './language-selection';

export interface EditorLanguageOption {
	value: string;
	label: string;
	dialect: Dialect;
}

export function createEditorLanguageOptions(
	catalog: readonly DialectInfo[],
): EditorLanguageOption[] {
	return LANGUAGE_OPTIONS.filter((option) => option.value !== 'system').map((option) => {
		const region = option.value.split('-')[1];
		const dialect = catalog.find((candidate) => candidate.code === region)?.dialect;

		if (dialect == null) {
			throw new Error(`The grammar engine does not include ${option.label}.`);
		}

		return {
			value: option.value,
			label: option.label,
			dialect,
		};
	});
}

export function resolveEditorLanguage(
	selection: LanguageSelection,
	options: readonly EditorLanguageOption[],
): EditorLanguageOption {
	const value = languageSelectionToSettingsValue(selection);
	const option = options.find((candidate) => candidate.value === value);

	if (!option) {
		throw new Error(`The editor does not support the resolved language: ${value}.`);
	}

	return option;
}
