import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  discoverFeeds: vi.fn(),
  previewFeed: vi.fn(),
  subscribeFeed: vi.fn(),
  createFolder: vi.fn(),
}));
vi.mock("./api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./api")>()),
  api,
}));

import { AddFeedFlow } from "./addFeed.svelte";

const preview = {
  url: "https://blog.example/feed.xml",
  title: "Blog",
  siteUrl: "https://blog.example/",
  items: [],
};

describe("AddFeedFlow", () => {
  beforeEach(() => {
    for (const fn of Object.values(api)) fn.mockReset();
    api.previewFeed.mockResolvedValue(preview);
    api.subscribeFeed.mockResolvedValue(7);
  });

  it("goes straight to the preview when there's one feed", async () => {
    api.discoverFeeds.mockResolvedValue([{ url: preview.url, title: "Blog" }]);
    const flow = new AddFeedFlow("blog.example");
    await flow.find();
    expect(api.discoverFeeds).toHaveBeenCalledWith("blog.example");
    expect(flow.step).toBe("preview");
    expect(flow.title).toBe("Blog");
  });

  it("asks which feed when a site has several", async () => {
    api.discoverFeeds.mockResolvedValue([
      { url: "https://blog.example/posts.xml", title: "Posts" },
      { url: "https://blog.example/comments.xml", title: "Comments" },
    ]);
    const flow = new AddFeedFlow("blog.example");
    await flow.find();
    expect(flow.step).toBe("choose");
    flow.chosenUrl = "https://blog.example/comments.xml";
    await flow.showPreview();
    expect(api.previewFeed).toHaveBeenCalledWith("https://blog.example/comments.xml");
    flow.back();
    expect(flow.step).toBe("choose");
  });

  it("subscribes with the chosen title and folder, creating the folder first", async () => {
    api.discoverFeeds.mockResolvedValue([{ url: preview.url, title: "Blog" }]);
    api.createFolder.mockResolvedValue(3);
    const flow = new AddFeedFlow("blog.example");
    await flow.find();
    flow.title = "  My blog  ";
    flow.folder = "new";
    flow.newFolderName = "Reading";
    expect(await flow.subscribe()).toBe(7);
    expect(api.createFolder).toHaveBeenCalledWith("Reading");
    expect(api.subscribeFeed).toHaveBeenCalledWith({
      url: preview.url,
      folderId: 3,
      title: "My blog",
    });
  });

  it("shows errors and returns to the step that failed", async () => {
    api.discoverFeeds.mockRejectedValue(new Error("No feed found"));
    const flow = new AddFeedFlow("nothing.example");
    await flow.find();
    expect(flow.step).toBe("address");
    expect(flow.error).toBe("No feed found");

    api.discoverFeeds.mockResolvedValue([{ url: preview.url, title: "Blog" }]);
    api.subscribeFeed.mockRejectedValue(new Error("You're already subscribed to this feed"));
    await flow.find();
    expect(await flow.subscribe()).toBeNull();
    expect(flow.step).toBe("preview");
    expect(flow.error).toContain("already subscribed");
  });

  it("does nothing without an address", async () => {
    await new AddFeedFlow("   ").find();
    expect(api.discoverFeeds).not.toHaveBeenCalled();
  });
});
