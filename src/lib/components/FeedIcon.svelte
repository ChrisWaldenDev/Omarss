<script lang="ts">
  import { letterAvatar } from "../format";

  let { title, size = 16 }: { title: string; size?: number } = $props();

  const avatar = $derived(letterAvatar(title));
</script>

<span
  class="feed-icon"
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{Math.round(size * 0.62)}px"
  style:background={avatar.color}
  aria-hidden="true">{avatar.letter}</span
>

<style>
  .feed-icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 4px;
    color: #fff;
    font-weight: 700;
    line-height: 1;
  }
</style>
