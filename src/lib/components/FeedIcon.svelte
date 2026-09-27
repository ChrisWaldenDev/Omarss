<script lang="ts">
  import { letterAvatar } from "../format";
  import { feedIconUrl } from "../icons";

  let {
    title,
    icon = null,
    size = 16,
  }: { title: string; icon?: string | null; size?: number } = $props();

  const avatar = $derived(letterAvatar(title));
  let failed = $state(false);

  // A new icon file gets a new chance to load.
  $effect(() => {
    void icon;
    failed = false;
  });
</script>

{#if icon && !failed}
  <img
    class="feed-icon"
    src={feedIconUrl(icon)}
    alt=""
    width={size}
    height={size}
    onerror={() => (failed = true)}
  />
{:else}
  <span
    class="feed-icon letter"
    style:width="{size}px"
    style:height="{size}px"
    style:font-size="{Math.round(size * 0.62)}px"
    style:background={avatar.color}
    aria-hidden="true">{avatar.letter}</span
  >
{/if}

<style>
  .feed-icon {
    flex: none;
    border-radius: 4px;
  }

  .letter {
    display: inline-grid;
    place-items: center;
    color: #fff;
    font-weight: 700;
    line-height: 1;
  }
</style>
