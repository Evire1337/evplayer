import { finalizeEvent, generateSecretKey, getPublicKey, nip44, Relay } from "nostr-tools";
import { readMessage, roomSecrets } from "./protocol";
import { JAM_RELAYS, type JamMessage } from "./types";

// The wire of a session: public Nostr relays. Every message is a short-lived event (the
// relays pass it on and forget it), tagged with a value made from the session code and
// encrypted with a key made from it too. Each copy of the app signs with its own key, so who
// sent a message is known for certain.

/** ephemeral events (20000–29999) are passed on and not stored */
const KIND = 20_001;
const CONNECT_TIMEOUT_MS = 5000;
const SEEN_LIMIT = 500;

export class JamLink {
  private readonly secret = generateSecretKey();
  /** this app's key in a session */
  readonly id = getPublicKey(this.secret);
  private relays: Relay[] = [];
  private subs: Array<{ close: () => void }> = [];
  private tag = "";
  private key: Uint8Array | null = null;
  // the same event comes from every relay
  private seen = new Set<string>();

  constructor(private readonly onMessage: (msg: JamMessage, from: string) => void) {}

  get open(): boolean {
    return this.key !== null;
  }

  /** start listening on the session with this code */
  async join(code: string): Promise<void> {
    const [{ tag, key }] = await Promise.all([roomSecrets(code), this.connect()]);
    for (const sub of this.subs) sub.close();
    this.subs = [];
    this.tag = tag;
    this.key = key;
    const since = Math.floor(Date.now() / 1000) - 5;
    for (const relay of this.relays) {
      this.subs.push(relay.subscribe([{ kinds: [KIND], "#t": [tag], since }], { onevent: (e) => this.receive(e.id, e.pubkey, e.content) }));
    }
  }

  async send(msg: JamMessage): Promise<void> {
    if (!this.key) return;
    const event = finalizeEvent(
      { kind: KIND, created_at: Math.floor(Date.now() / 1000), tags: [["t", this.tag]], content: nip44.v2.encrypt(JSON.stringify(msg), this.key) },
      this.secret,
    );
    this.seen.add(event.id);
    await Promise.allSettled(this.relays.map((r) => r.publish(event)));
  }

  /** text only `to` can read (the new code, when the session moves) */
  seal(text: string, to: string): string {
    return nip44.v2.encrypt(text, nip44.v2.utils.getConversationKey(this.secret, to));
  }

  unseal(sealed: string, from: string): string | null {
    try {
      return nip44.v2.decrypt(sealed, nip44.v2.utils.getConversationKey(this.secret, from));
    } catch {
      return null;
    }
  }

  close(): void {
    for (const sub of this.subs) sub.close();
    for (const relay of this.relays) relay.close();
    this.subs = [];
    this.relays = [];
    this.key = null;
    this.tag = "";
    this.seen.clear();
  }

  private async connect(): Promise<void> {
    if (this.relays.some((r) => r.connected)) return;
    const tries = await Promise.allSettled(JAM_RELAYS.map((url) => Relay.connect(url, { enablePing: true, enableReconnect: true, timeout: CONNECT_TIMEOUT_MS })));
    this.relays = tries.flatMap((t) => (t.status === "fulfilled" ? [t.value] : []));
    if (!this.relays.length) throw new Error("Не удалось подключиться к серверам сессий. Проверьте интернет.");
  }

  private receive(eventId: string, from: string, content: string): void {
    if (this.seen.has(eventId) || from === this.id || !this.key) return;
    this.seen.add(eventId);
    if (this.seen.size > SEEN_LIMIT) this.seen.delete(this.seen.values().next().value!);
    let text: string;
    try {
      text = nip44.v2.decrypt(content, this.key);
    } catch {
      return; // not from this session (or the session moved since)
    }
    const msg = readMessage(text);
    if (msg) this.onMessage(msg, from);
  }
}
