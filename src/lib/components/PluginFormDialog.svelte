<script lang="ts">
  // Generic "fill in a few fields" popup for the plugin API's
  // ui.showForm() - the shell (overlay/panel/Escape/click-outside) mirrors
  // ConfirmDialog.svelte; a plugin declares its fields and this renders
  // them, with no plugin-specific markup here.
  import type { PluginFormField } from '../plugins/pluginApi';

  export let title: string;
  export let fields: PluginFormField[];
  export let submitLabel = 'Submit';
  export let onSubmit: (values: Record<string, string>) => void;
  export let onClose: () => void;

  let panelEl: HTMLDivElement;
  let values: Record<string, string> = Object.fromEntries(
    fields.map((field) => [field.key, field.defaultValue ?? (field.type === 'select' ? (field.options?.[0]?.value ?? '') : '')]),
  );

  // <select>/<option> render as unstyled native OS popups on Linux
  // (WebKitGTK) - a custom dropdown (mirroring SettingsPanel's palette
  // picker) is the only way to get themed colors here. Only one open at a
  // time, tracked by field key.
  let openDropdown: string | null = null;

  const optionLabel = (field: PluginFormField): string =>
    field.options?.find((option) => option.value === values[field.key])?.label ?? field.placeholder ?? 'Select…';

  const toggleDropdown = (key: string) => {
    openDropdown = openDropdown === key ? null : key;
  };

  const chooseOption = (field: PluginFormField, value: string) => {
    values[field.key] = value;
    openDropdown = null;
  };

  $: canSubmit = fields.every((field) => !field.required || values[field.key]?.trim());

  const submit = () => {
    if (!canSubmit) return;
    onSubmit(values);
    onClose();
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    } else if (event.key === 'Enter' && !(event.target instanceof HTMLTextAreaElement) && !event.shiftKey) {
      // Textareas keep Enter for newlines - only single-line fields submit.
      event.preventDefault();
      submit();
    }
  };

  const handleDropdownOutsideClick = (event: MouseEvent) => {
    if (!(event.target as HTMLElement).closest('.dropdown')) openDropdown = null;
  };

  const handleOutsideClick = (event: MouseEvent) => {
    if (panelEl && !panelEl.contains(event.target as Node)) onClose();
  };
</script>

<svelte:window on:keydown={handleKeydown} on:mousedown={handleDropdownOutsideClick} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" on:mousedown={handleOutsideClick}>
  <div class="panel" bind:this={panelEl} role="dialog" aria-modal="true" aria-label={title}>
    <h2 class="title">{title}</h2>
    <div class="fields">
      {#each fields as field (field.key)}
        <label class="field">
          <span>{field.label}{field.required ? ' *' : ''}</span>
          {#if field.type === 'textarea'}
            <textarea bind:value={values[field.key]} placeholder={field.placeholder} rows="4"></textarea>
          {:else if field.type === 'select'}
            <div class="dropdown">
              <button
                class="select"
                type="button"
                on:click={() => toggleDropdown(field.key)}
                aria-haspopup="listbox"
                aria-expanded={openDropdown === field.key}
              >
                {optionLabel(field)}
                <svg class="caret" width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M2.5 3.5L5 6.5L7.5 3.5" />
                </svg>
              </button>
              {#if openDropdown === field.key}
                <ul class="dropdown-menu" role="listbox">
                  {#each field.options ?? [] as option (option.value)}
                    <li>
                      <button
                        class="dropdown-item"
                        class:active={option.value === values[field.key]}
                        role="option"
                        aria-selected={option.value === values[field.key]}
                        on:click={() => chooseOption(field, option.value)}
                      >
                        {option.label}
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {:else}
            <input type="text" bind:value={values[field.key]} placeholder={field.placeholder} />
          {/if}
        </label>
      {/each}
    </div>
    <div class="actions">
      <button class="btn" on:click={onClose}>Cancel</button>
      <button class="btn primary" on:click={submit} disabled={!canSubmit}>{submitLabel}</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: var(--window-shadow-margin, 0);
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1200;
  }

  .panel {
    width: min(360px, 90vw);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
    padding: 1rem;
  }

  .title {
    margin: 0 0 0.75rem;
    font-size: 0.9rem;
    color: var(--text);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin-bottom: 1rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .field input,
  .field textarea {
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--text);
    padding: 0.4rem 0.55rem;
    resize: vertical;
  }

  .field input:focus,
  .field textarea:focus {
    outline: 1px solid var(--accent);
  }

  .dropdown {
    position: relative;
  }

  .select {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
    width: 100%;
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--text);
    padding: 0.4rem 0.55rem;
    cursor: pointer;
  }

  .select:hover {
    background: var(--accent-soft, var(--panel-2));
  }

  .select .caret {
    color: var(--muted);
    flex-shrink: 0;
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 0.25rem);
    left: 0;
    right: 0;
    z-index: 10;
    list-style: none;
    margin: 0;
    padding: 0.25rem;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-height: 12rem;
    overflow-y: auto;
  }

  .dropdown-item {
    display: block;
    width: 100%;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    padding: 0.35rem 0.6rem;
    font-size: 0.78rem;
    border-radius: 0.3rem;
    cursor: pointer;
  }

  .dropdown-item:hover {
    background: var(--panel);
  }

  .dropdown-item.active {
    color: var(--accent);
    font-weight: 600;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .btn {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.8rem;
    padding: 0.35rem 0.85rem;
    cursor: pointer;
  }

  .btn:hover {
    background: var(--border);
  }

  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .btn.primary:hover {
    filter: brightness(1.1);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
