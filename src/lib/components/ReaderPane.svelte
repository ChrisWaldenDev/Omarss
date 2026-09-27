<script lang="ts">
  import { api, errorMessage } from "../api";
  import { fileSize, fullDate } from "../format";
  import { t } from "../i18n";
  import { articles } from "../stores/app.svelte";
  import { toasts } from "../stores/toasts.svelte";
  import Icon from "./Icon.svelte";

  const article = $derived(articles.article);
  let scroller = $state<HTMLElement>();
  let content = $state<HTMLElement>();

  // Start each article at the top.
  $effect(() => {
    if (article && scroller) scroller.scrollTop = 0;
  });

  // Remote images load through the image proxy (a later milestone); until then the content
  // security policy blocks them, so hide them instead of showing broken-image icons.
  $effect(() => {
    void article?.contentHtml;
    for (const img of content?.querySelectorAll("img") ?? []) {
      img.addEventListener("error", () => img.classList.add("blocked"), { once: true });
    }
  });

  function act(action: Promise<void>) {
    action.catch((error) => toasts.show(t("error.action", { message: errorMessage(error) })));
  }

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
        <div class="toolbar">
          {#if article.url}
            {@const url = article.url}
            <button class="tool" onclick={() => openExternal(url)}>
              <Icon name="external" size={14} />
              {t("reader.openOriginal")}
            </button>
          {/if}
          <button
            class="tool"
            aria-pressed={article.isStarred}
            onclick={() => act(articles.toggleStar(article.id))}
          >
            <span class="star-icon" class:on={article.isStarred}>
              <Icon name="star" size={14} filled={article.isStarred} />
            </span>
            {article.isStarred ? t("reader.unstar") : t("reader.star")}
          </button>
          <button class="tool" onclick={() => act(articles.setRead(article.id, !article.isRead))}>
            <Icon name={article.isRead ? "mail" : "mailOpen"} size={14} />
            {article.isRead ? t("reader.markUnread") : t("reader.markRead")}
          </button>
        </div>
      </header>

      <!-- Content is sanitised by the backend before it is stored (SPEC §8.3). -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="content" bind:this={content} onclick={onContentClick}>
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

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .tool {
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

  .tool:hover {
    border-color: var(--accent);
  }

  .star-icon {
    display: grid;
  }

  .star-icon.on {
    color: var(--star);
  }

  .content :global(img.blocked) {
    display: none;
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
