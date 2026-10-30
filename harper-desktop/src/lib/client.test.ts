import { beforeEach, expect, test, vi } from 'vitest';
import { Client } from './client';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-autostart', () => ({
	disable: vi.fn(),
	enable: vi.fn(),
	isEnabled: vi.fn(),
}));

beforeEach(() => {
	invoke.mockReset();
});

test('loads and validates the language selection from Tauri', async () => {
	invoke.mockResolvedValue({ mode: 'explicit', language: 'de', region: 'DE' });

	await expect(Client.getLanguageSelection()).resolves.toEqual({
		mode: 'explicit',
		language: 'de',
		region: 'DE',
	});
	expect(invoke).toHaveBeenCalledWith('get_language_selection');
});

test('sends the selected language to Tauri', async () => {
	invoke.mockResolvedValue(undefined);
	const selection = { mode: 'explicit', language: 'de', region: 'AT' } as const;

	await Client.setLanguageSelection(selection);

	expect(invoke).toHaveBeenCalledWith('set_language_selection', {
		languageSelection: selection,
	});
});

test('loads the resolved editor language from Tauri', async () => {
	invoke.mockResolvedValue({ mode: 'explicit', language: 'de', region: 'CH' });

	await expect(Client.getResolvedLanguageSelection()).resolves.toEqual({
		mode: 'explicit',
		language: 'de',
		region: 'CH',
	});
	expect(invoke).toHaveBeenCalledWith('get_resolved_language_selection');
});

test('rejects an unresolved editor language from Tauri', async () => {
	invoke.mockResolvedValue({ mode: 'system', language: null, region: null });

	await expect(Client.getResolvedLanguageSelection()).rejects.toThrow(
		'Resolved language selection must be explicit.',
	);
});
