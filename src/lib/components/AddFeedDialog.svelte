<script lang="ts">
  import { AddFeedFlow } from "../addFeed.svelte";
  import { relativeTime } from "../format";
  import { t } from "../i18n";
  import { articles } from "../stores/app.svelte";
  import { clock } from "../stores/clock.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import Dialog from "./Dialog.svelte";

  let { onclose, address = "" }: { onclose: () => void; address?: string } = $props();

  // svelte-ignore state_referenced_locally
  const flow = new AddFeedFlow(address);
  const folders = $derived(sidebar.data?.folders ?? []);

  async function subscribe() {
    const id = await flow.subscribe();
    if (id === null) return;
    onclose();
    await sidebar.load();
    await articles.showView({ kind: "feed", id });
  }
</script>

<Dialog title={t("addFeed.title")} {onclose} wide>
  {#if flow.error}
    <p class="form-error" role="alert">{flow.error}</p>
  {/if}

  {#if flow.step === "address" || flow.step === "discovering"}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        flow.find();
      }}
    >
      <label class="field">
        <span>{t("addFeed.address")}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="text"
          bind:value={flow.address}
          placeholder="example.com"
          autocomplete="off"
          spellcheck="false"
          autofocus
          disabled={flow.busy}
        />
      </label>
      <p class="hint">{t("addFeed.addressHint")}</p>
      <div class="actions">
        <button class="button primary" type="submit" disabled={flow.busy || !flow.address.trim()}>
          {flow.step === "discovering" ? t("addFeed.finding") : t("addFeed.find")}
        </button>
      </div>
    </form>
  {:else if flow.step === "choose" || (flow.step === "previewing" && flow.candidates.length > 1)}
    <fieldset class="choices">
      <legend>{t("addFeed.choose")}</legend>
      {#each flow.candidates as candidate (candidate.url)}
        <label class="choice">
          <input type="radio" name="feed" value={candidate.url} bind:group={flow.chosenUrl} />
          <span>
            <strong>{candidate.title ?? candidate.url}</strong>
            {#if candidate.title}<small>{candidate.url}</small>{/if}
          </span>
        </label>
      {/each}
    </fieldset>
    <div class="actions">
      <button class="button" onclick={() => flow.back()} disabled={flow.busy}>
        {t("addFeed.back")}
      </button>
      <button class="button primary" onclick={() => flow.showPreview()} disabled={flow.busy}>
        {flow.step === "previewing" ? t("addFeed.loadingPreview") : t("addFeed.preview")}
      </button>
    </div>
  {:else if flow.step === "previewing"}
    <p class="hint">{t("addFeed.loadingPreview")}</p>
  {:else if flow.preview}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        subscribe();
      }}
    >
      <p class="feed-url">{flow.preview.url}</p>
      <div class="latest">
        <h3>{t("addFeed.latest")}</h3>
        {#if flow.preview.items.length === 0}
          <p class="hint">{t("addFeed.noItems")}</p>
        {:else}
          <ul>
            {#each flow.preview.items as item, i (i)}
              <li>
                <span class="item-title">{item.title}</span>
                {#if item.publishedAt !== null}
                  <span class="item-time">{relativeTime(item.publishedAt, clock.now)}</span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
      <label class="field">
        <span>{t("addFeed.name")}</span>
        <input type="text" bind:value={flow.title} disabled={flow.busy} />
      </label>
      <label class="field">
        <span>{t("addFeed.folder")}</span>
        <select bind:value={flow.folder} disabled={flow.busy}>
          <option value={null}>{t("addFeed.noFolder")}</option>
          {#each folders as folder (folder.id)}
            <option value={folder.id}>{folder.name}</option>
          {/each}
          <option value="new">{t("addFeed.newFolder")}</option>
        </select>
      </label>
      {#if flow.folder === "new"}
        <label class="field">
          <span>{t("addFeed.newFolderName")}</span>
          <input type="text" bind:value={flow.newFolderName} disabled={flow.busy} />
        </label>
      {/if}
      <div class="actions">
        <button class="button" type="button" onclick={() => flow.back()} disabled={flow.busy}>
          {t("addFeed.back")}
        </button>
        <button
          class="button primary"
          type="submit"
          disabled={flow.busy || (flow.folder === "new" && !flow.newFolderName.trim())}
        >
          {flow.step === "subscribing" ? t("addFeed.subscribing") : t("addFeed.subscribe")}
        </button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .hint {
    margin: -4px 0 14px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0 0 16px;
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: 8px;
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .choice:has(input:checked) {
    border-color: var(--accent);
    background: var(--bg-selected);
  }

  .choice span {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .choice small,
  .feed-url {
    overflow: hidden;
    color: var(--text-muted);
    font-size: 0.8rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .feed-url {
    margin: -4px 0 12px;
  }

  .latest {
    margin-bottom: 16px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--bg-sidebar);
  }

  h3 {
    margin: 0 0 6px;
    color: var(--text-muted);
    font-size: 0.75rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    gap: 12px;
    padding: 3px 0;
    font-size: 0.88rem;
  }

  .item-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-time {
    flex: none;
    color: var(--text-muted);
    font-size: 0.8rem;
  }
</style>
