import { useSettings } from "@/settings/store";
import { resolveLang, setCurrentLang, type Lang } from "./lang";
import { translate } from "./translate";

// The page is written in Russian; in English every text on the screen is swapped for its
// dictionary entry in place (the text nodes and the attributes people read: labels, hints,
// placeholders), and again whenever the app writes new text. Switching back puts the Russian
// text back. React only ever sets a text to what it wants, so a text this file has changed is
// not fought over: the next write from React is a Russian text, which is translated again.

const ATTRIBUTES = ["aria-label", "placeholder", "title", "data-tip", "alt"] as const;

/**
 * What the app shows as content, not as interface, stays as it is: songs, artists and lyrics
 * (a Russian song line like "Нет" must not become "No"). Only the app's own words are translated.
 */
/** the made-for-you cards reuse the song-card classes for words of the app's own */
const APP_OWN = ".smart-card";
const CONTENT = "[data-no-translate], script, style, textarea, canvas, .lyrics, .lyric-line, .track-title, .track-artist, .track-card-title, .track-card-artist, .queue-name, .queue-sub, .artist-name";

/** the Russian original of a text node, and what it was last turned into */
const texts = new WeakMap<Text, { original: string; shown: string }>();
const attributes = new WeakMap<Element, Map<string, { original: string; shown: string }>>();
/**
 * React writes "46 из 190 · 24%" as six text nodes in a row. Read piece by piece it is not
 * a text anyone translated, so a run of text nodes (two or more side by side) is read as a
 * whole: the whole goes into its first node and the others are emptied.
 */
interface Group {
  nodes: Text[];
  originals: string[];
  shown: string;
}
/** by the first node of the run */
const groups = new WeakMap<Text, Group>();
const merged = new WeakSet<Text>();
const touchedGroups = new Set<WeakRef<Text>>();
/** every node touched, to put the Russian back (a WeakRef: nodes that go away are not held) */
const touched = new Set<WeakRef<Text | Element>>();

let lang: Lang = "ru";
let observer: MutationObserver | null = null;

/** the Russian text of a node, whatever this file has made of it */
function originalOf(node: Text): string {
  const known = texts.get(node);
  return known && node.data === known.shown ? known.original : node.data;
}

/** the runs of text nodes, two or more side by side, among the children of an element */
function runsOf(parent: Element): Text[][] {
  const runs: Text[][] = [];
  let run: Text[] = [];
  for (const kid of Array.from(parent.childNodes)) {
    if (kid.nodeType === Node.TEXT_NODE) run.push(kid as Text);
    else {
      if (run.length > 1) runs.push(run);
      run = [];
    }
  }
  if (run.length > 1) runs.push(run);
  return runs;
}

/** true when the run is translated as a whole (and written) */
function applyRun(nodes: Text[]): boolean {
  let group = groups.get(nodes[0]);
  // other nodes than the ones merged before: start again
  if (group && (group.nodes.length !== nodes.length || group.nodes.some((n, i) => n !== nodes[i]))) {
    group.nodes.forEach((n) => merged.delete(n));
    groups.delete(nodes[0]);
    group = undefined;
  }
  let originals: string[];
  if (group) {
    // what React wrote since: the first node holds the translation, the others are empty
    const last = group;
    originals = last.originals.slice();
    nodes.forEach((n, i) => {
      if (n.data !== (i === 0 ? last.shown : "")) originals[i] = n.data;
    });
  } else {
    originals = nodes.map(originalOf);
  }
  const combined = originals.join("");
  const shown = translate(combined, lang);
  if (shown === combined) {
    if (group) {
      nodes.forEach((n, i) => {
        n.data = originals[i];
        merged.delete(n);
      });
      groups.delete(nodes[0]);
    }
    return false;
  }
  groups.set(nodes[0], { nodes, originals, shown });
  touchedGroups.add(new WeakRef(nodes[0]));
  nodes.forEach((n, i) => {
    merged.add(n);
    const want = i === 0 ? shown : "";
    if (n.data !== want) n.data = want;
  });
  return true;
}

function applyGroup(parent: Element): void {
  for (const run of runsOf(parent)) applyRun(run);
}

/** songs, artists and lyrics: shown as they are */
function isContent(el: Element): boolean {
  const hit = el.closest(CONTENT);
  return hit !== null && !hit.closest(APP_OWN);
}

function applyText(node: Text): void {
  // part of a run of text nodes: the run is read as a whole
  if (node.parentElement) applyGroup(node.parentElement);
  if (merged.has(node)) return;
  const known = texts.get(node);
  // the text is what this file wrote last: nothing to do
  if (known && node.data === known.shown) return;
  const original = node.data;
  const shown = translate(original, lang);
  if (shown === original && !known) return;
  texts.set(node, { original, shown });
  touched.add(new WeakRef(node));
  if (shown !== node.data) node.data = shown;
}

function applyAttribute(el: Element, name: string): void {
  const value = el.getAttribute(name);
  if (value === null) return;
  let map = attributes.get(el);
  const known = map?.get(name);
  if (known && value === known.shown) return;
  const shown = translate(value, lang);
  if (shown === value && !known) return;
  if (!map) attributes.set(el, (map = new Map()));
  map.set(name, { original: value, shown });
  touched.add(new WeakRef(el));
  if (shown !== value) el.setAttribute(name, shown);
}

function walk(root: Node): void {
  if (root.nodeType === Node.TEXT_NODE) return applyText(root as Text);
  if (root.nodeType !== Node.ELEMENT_NODE) return;
  const el = root as Element;
  // what the app shows as data (a song's title, a name someone typed) is not interface text
  if (isContent(el)) return;
  for (const name of ATTRIBUTES) if (el.hasAttribute(name)) applyAttribute(el, name);
  applyGroup(el);
  for (const child of Array.from(el.childNodes)) walk(child);
}

function onMutations(records: MutationRecord[]): void {
  for (const r of records) {
    if (r.type === "characterData") {
      if (r.target.parentElement && isContent(r.target.parentElement)) continue;
      applyText(r.target as Text);
    } else if (r.type === "attributes") applyAttribute(r.target as Element, r.attributeName!);
    else {
      r.addedNodes.forEach(walk);
      // a text node came or went: the run it was part of reads differently now
      if (r.target.nodeType === Node.ELEMENT_NODE && !isContent(r.target as Element)) applyGroup(r.target as Element);
    }
  }
}

/** puts the Russian text back everywhere this file changed it */
function restore(): void {
  for (const ref of touchedGroups) {
    const first = ref.deref();
    const group = first && groups.get(first);
    if (group) {
      group.nodes.forEach((n, i) => {
        n.data = group.originals[i];
        merged.delete(n);
      });
    }
    if (first) groups.delete(first);
  }
  touchedGroups.clear();
  for (const ref of touched) {
    const node = ref.deref();
    if (!node) {
      touched.delete(ref);
      continue;
    }
    if (node.nodeType === Node.TEXT_NODE) {
      const known = texts.get(node as Text);
      if (known && (node as Text).data === known.shown) (node as Text).data = known.original;
      texts.delete(node as Text);
    } else {
      const map = attributes.get(node as Element);
      for (const [name, known] of map ?? []) {
        if ((node as Element).getAttribute(name) === known.shown) (node as Element).setAttribute(name, known.original);
      }
      attributes.delete(node as Element);
    }
  }
  touched.clear();
}

function setLang(next: Lang): void {
  if (next === lang && (observer !== null || next === "ru")) return;
  observer?.disconnect();
  observer = null;
  if (lang === "en") restore();
  lang = next;
  setCurrentLang(next);
  document.documentElement.lang = next;
  if (next === "en") {
    walk(document.body);
    observer = new MutationObserver(onMutations);
    observer.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: [...ATTRIBUTES] });
  }
}

/** follows the language setting for the life of the page */
export function startTranslator(): () => void {
  const apply = () => setLang(resolveLang(useSettings.getState().language));
  apply();
  const stop = useSettings.subscribe((s, prev) => {
    if (s.language !== prev.language) apply();
  });
  // the system language can change while the app runs
  window.addEventListener("languagechange", apply);
  return () => {
    stop();
    window.removeEventListener("languagechange", apply);
    setLang("ru");
  };
}
