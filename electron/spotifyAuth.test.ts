import { createHash } from "node:crypto";
import { describe, expect, it, vi } from "vitest";
vi.mock("electron", () => ({ shell: { openExternal: vi.fn() } }));
import { pkcePair } from "./spotifyAuth";

describe("spotify pkce", () => {
  it("makes a verifier and its S256 challenge", () => {
    const { verifier, challenge } = pkcePair();
    expect(verifier).toMatch(/^[A-Za-z0-9_-]{43,128}$/);
    const expected = createHash("sha256").update(verifier).digest("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    expect(challenge).toBe(expected);
  });
});
