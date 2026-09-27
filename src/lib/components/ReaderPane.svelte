<script lang="ts">
  import { api } from "../api";
  import { fileSize, fullDate } from "../format";
  import { t } from "../i18n";
  import { articles } from "../stores/articles.svelte";
  import Icon from "./Icon.svelte";

  const article = $derived(articles.article);
  let scroller = $state<HTMLElement>();

  // Start each article at the top.
  $effect(() => {
    if (article && scroller) scroller.scrollTop = 0;
  });

  function openExternal(url: string) {
    api.openExternal(url).catch((error) => console.error("open_external failed", error));
  }

  /** Links in article content open in the OS browser, never inside the app (SPEC §8.2). */
  function onContentClick(event: MouseEvent) {
    const link = (event.target as Element | null)?.closest("a[href]");
    if (!(link instanceof HTMLAnchorElement)) return;
    event.preventDefault();
    openExternal(link.href);
  }
</script>

<section class="reader" aria-label={t("reader.label")} bind:this={scroller}>
  {#if articles.articleError}
    <p class="message error" role="alert">
      {t("reader.loadError", { message: articles.articleError })}
    </p>
  {:else if article}
    <article class="article">
      <header class="header">
        <p class="feed">{article.feedTitle}</p>
        <h1 class="title">{article.title}</h1>
        <p class="byline">
          {#if article.author}
            <span>{t("reader.byAuthor", { author: article.author })}</span>
          {/if}
          {#if article.publishedAt !== null}
            <time datetime={new Date(article.publishedAt * 1000).toISOString()}>
              {fullDate(article.publishedAt)}
            </time>
          {/if}
        </p>
        {#if article.url}
          {@const url = article.url}
          <button class="open" onclick={() => openExternal(url)}>
            <Icon name="external" size={14} />
            {t("reader.openOriginal")}
          </button>
        {/if}
      </header>

      <!-- Content is sanitised by the backend before it is stored (SPEC §8.3). -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="content" onclick={onContentClick}>
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        {@html article.contentHtml}
      </div>

      {#if article.enclosures.length > 0}
        <section class="attachments">
          <h2>{t("reader.attachments")}</h2>
          <ul>
            {#each article.enclosures as enclosure (enclosure.url)}
              <li>
                <button class="link" onclick={() => openExternal(enclosure.url)}>
                  <Icon name="attachment" size={14} />
                  <span class="file">{enclosure.url.split("/").pop()}</span>
                </button>
                <span class="details">
                  {[enclosure.mimeType, enclosure.length ? fileSize(enclosure.length) : null]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </article>
  {:else}
    <p class="message">{t("reader.empty")}</p>
  {/if}
</section>

<style>
  .reader {
    min-width: 0;
    overflow-y: auto;
    background: var(--bg);
  }

  .message {
    margin: 30vh 24px 0;
    color: var(--text-muted);
    text-align: center;
  }

  .message.error {
    color: var(--danger);
  }

  .article {
    max-width: var(--reader-width);
    margin: 0 auto;
    padding: 32px 32px 64px;
  }

  .header {
    margin-bottom: 28px;
    padding-bottom: 20px;
    border-bottom: 1px solid var(--border);
  }

  .feed {
    margin: 0 0 6px;
    color: var(--accent-text);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .title {
    margin: 0 0 10px;
    font-size: 1.75rem;
    line-height: 1.25;
    letter-spacing: -0.01em;
  }

  .byline {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    margin: 0 0 14px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .open {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-input);
    color: var(--text);
    font-size: 0.85rem;
  }

  .open:hover {
    border-color: var(--accent);
  }

  .content {
    font-size: var(--reader-font-size);
    line-height: var(--reader-line-height);
    overflow-wrap: break-word;
  }

  .content :global(:is(h1, h2, h3, h4)) {
    margin: 1.6em 0 0.6em;
    line-height: 1.3;
  }

  .content :global(p),
  .content :global(ul),
  .content :global(ol) {
    margin: 0 0 1em;
  }

  .content :global(a) {
    color: var(--accent-text);
  }

  .content :global(img),
  .content :global(video) {
    max-width: 100%;
    height: auto;
  }

  .content :global(blockquote) {
    margin: 1.2em 0;
    padding: 0.2em 1em;
    border-left: 3px solid var(--accent);
    color: var(--text-muted);
  }

  .content :global(pre) {
    overflow-x: auto;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--bg-code);
    font-size: 0.85em;
    line-height: 1.5;
  }

  .content :global(code) {
    font-family: var(--font-mono);
  }

  .attachments {
    margin-top: 32px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }

  .attachments h2 {
    margin: 0 0 8px;
    font-size: 0.9rem;
  }

  .attachments ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .attachments li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    padding: 4px 0;
  }

  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent-text);
    text-decoration: underline;
  }

  .details {
    color: var(--text-muted);
    font-size: 0.8rem;
  }
</style>
