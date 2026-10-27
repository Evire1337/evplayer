// @vitest-environment node
import { describe, expect, it, vi } from "vitest";
vi.mock("electron", () => ({ app: { getPath: () => "/tmp" }, net: { fetch: vi.fn() } }));
import { extractClientId, scriptUrls } from "./soundcloudClientId";

describe("soundcloud client id", () => {
  it("finds the bundles and the id in them", () => {
    const html = '<script crossorigin src="https://a-v2.sndcdn.com/assets/0-abc.js"></script><script src="https://a-v2.sndcdn.com/assets/49-def.js"></script>';
    expect(scriptUrls(html)).toEqual(["https://a-v2.sndcdn.com/assets/0-abc.js", "https://a-v2.sndcdn.com/assets/49-def.js"]);
    expect(extractClientId('x={client_id:"AbCdEfGhIjKlMnOpQrStUvWxYz012345",y:1}')).toBe("AbCdEfGhIjKlMnOpQrStUvWxYz012345");
    expect(extractClientId("nothing here")).toBeNull();
  });
});
