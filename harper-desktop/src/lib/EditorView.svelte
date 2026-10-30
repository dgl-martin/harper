<script lang="ts">
import type { Linter } from 'harper.js';
import { Editor } from 'harper-editor';
import { onDestroy, onMount } from 'svelte';
import 'harper-editor/style.css';
import { Client } from './client';
import {
	createEditorLanguageOptions,
	type EditorLanguageOption,
	resolveEditorLanguage,
} from './editor-language';

let linter: Linter | null = null;
let languageOptions: EditorLanguageOption[] = [];
let language = '';
let linterRevision = 0;
let languageChangeVersion = 0;
let changingLanguage = false;
let errorMessage: string | null = null;
let destroyed = false;

onMount(() => {
	void createLinter().catch(showError);
});

onDestroy(() => {
	destroyed = true;
	void linter?.dispose();
});

async function createLinter() {
	const [{ WorkerLinter }, { binaryInlined }, resolvedSelection] = await Promise.all([
		import('harper.js'),
		import('harper.js/binaryInlined'),
		Client.getResolvedLanguageSelection(),
	]);

	const options = createEditorLanguageOptions(await binaryInlined.getDialectCatalog());
	const initialLanguage = resolveEditorLanguage(resolvedSelection, options);
	const nextLinter = new WorkerLinter({
		binary: binaryInlined,
		dialect: initialLanguage.dialect,
	});
	await nextLinter.setup();

	if (destroyed) {
		await nextLinter.dispose();
		return;
	}

	languageOptions = options;
	language = initialLanguage.value;
	linter = nextLinter;
}

async function setEditorLanguage(value: string) {
	const option = languageOptions.find((candidate) => candidate.value === value);

	if (!option || !linter || value === language) {
		return;
	}

	const version = ++languageChangeVersion;
	changingLanguage = true;
	errorMessage = null;

	try {
		await linter.setDialect(option.dialect);

		if (version === languageChangeVersion) {
			language = option.value;
			linterRevision += 1;
		}
	} catch (error) {
		showError(error);
	} finally {
		if (version === languageChangeVersion) {
			changingLanguage = false;
		}
	}
}

function showError(error: unknown) {
	errorMessage = error instanceof Error ? error.message : String(error);
	console.error('Failed to configure the editor language', error);
}
</script>

<div class="relative h-screen w-screen">
	{#if linter}
		<Editor
			{linter}
			{linterRevision}
			{languageOptions}
			{language}
			languageDisabled={changingLanguage}
			onLanguageChange={(value) => void setEditorLanguage(value)}
		/>
	{:else if errorMessage}
		<div class="flex h-full items-center justify-center p-8 text-center text-sm text-red-700" role="alert">
			{errorMessage}
		</div>
	{:else}
		<div class="flex h-full items-center justify-center text-sm text-stone-500">
			Loading Harper's multilingual grammar engine...
		</div>
	{/if}

	{#if linter && errorMessage}
		<div
			class="absolute top-3 left-1/2 z-30 -translate-x-1/2 rounded-md bg-red-50 px-3 py-2 text-sm text-red-700 shadow"
			role="alert"
		>
			{errorMessage}
		</div>
	{/if}
</div>
