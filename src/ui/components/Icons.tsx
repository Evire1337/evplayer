import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 20, children, ...rest }: P & { children: React.ReactNode }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.9} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...rest}>
      {children}
    </svg>
  );
}

export const IconPlay = (p: P) => <Svg {...p}><path d="M7 4.5v15l13-7.5z" fill="currentColor" stroke="none" /></Svg>;
export const IconPause = (p: P) => <Svg {...p}><rect x="6" y="4.5" width="4" height="15" rx="1.2" fill="currentColor" stroke="none" /><rect x="14" y="4.5" width="4" height="15" rx="1.2" fill="currentColor" stroke="none" /></Svg>;
export const IconNext = (p: P) => <Svg {...p}><path d="M5 5l10 7-10 7z" fill="currentColor" stroke="none" /><path d="M18.5 5v14" /></Svg>;
export const IconPrev = (p: P) => <Svg {...p}><path d="M19 5L9 12l10 7z" fill="currentColor" stroke="none" /><path d="M5.5 5v14" /></Svg>;
export const IconShuffle = (p: P) => <Svg {...p}><path d="M3 7h3.5c4.5 0 6.5 10 11 10H21" /><path d="M3 17h3.5c1.6 0 2.8-1.2 3.8-2.8" /><path d="M13.7 9.8C14.7 8.2 15.9 7 17.5 7H21" /><path d="M18.5 4.5L21 7l-2.5 2.5M18.5 14.5L21 17l-2.5 2.5" /></Svg>;
export const IconRepeat = (p: P) => <Svg {...p}><path d="M4 12V9.5A3.5 3.5 0 0 1 7.5 6H19" /><path d="M16.5 3.5L19 6l-2.5 2.5" /><path d="M20 12v2.5a3.5 3.5 0 0 1-3.5 3.5H5" /><path d="M7.5 20.5L5 18l2.5-2.5" /></Svg>;
export const IconVolume = (p: P) => <Svg {...p}><path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" stroke="none" /><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" /></Svg>;
export const IconMute = (p: P) => <Svg {...p}><path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" stroke="none" /><path d="M16 9.5l5 5M21 9.5l-5 5" /></Svg>;
export const IconHome = (p: P) => <Svg {...p}><path d="M4 10.5L12 4l8 6.5V19a1.5 1.5 0 0 1-1.5 1.5h-4v-6h-5v6h-4A1.5 1.5 0 0 1 4 19z" /></Svg>;
export const IconSearch = (p: P) => <Svg {...p}><circle cx="11" cy="11" r="6.5" /><path d="M20 20l-4.2-4.2" /></Svg>;
export const IconLibrary = (p: P) => <Svg {...p}><path d="M5 4v16M10 4v16" /><path d="M14.5 4.8l4.6 14.4" /></Svg>;
export const IconClose = (p: P) => <Svg {...p}><path d="M6 6l12 12M18 6L6 18" /></Svg>;
export const IconDisc = (p: P) => <Svg {...p}><circle cx="12" cy="12" r="8.5" /><circle cx="12" cy="12" r="2.5" /></Svg>;
export const IconSliders = (p: P) => <Svg {...p}><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle cx="10" cy="17" r="2" /></Svg>;
export const IconLyrics = (p: P) => <Svg {...p}><path d="M5 6h14M5 11h10M5 16h7" /><path d="M17 14v5.5a1.5 1.5 0 1 1-1.5-1.5H17" /></Svg>;
export const IconQueue = (p: P) => <Svg {...p}><path d="M4 6h12M4 11h12M4 16h7" /><path d="M16 14l4 2.5-4 2.5z" fill="currentColor" stroke="none" /></Svg>;
export const IconMinus = (p: P) => <Svg {...p}><path d="M6 12h12" /></Svg>;
export const IconPlus = (p: P) => <Svg {...p}><path d="M12 6v12M6 12h12" /></Svg>;
export const IconExpand = (p: P) => <Svg {...p}><path d="M9 4H4v5M15 4h5v5M9 20H4v-5M15 20h5v-5" /></Svg>;
export const IconHeart = (p: P) => <Svg {...p}><path d="M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z" /></Svg>;
export const IconHeartFill = (p: P) => <Svg {...p}><path d="M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z" fill="currentColor" /></Svg>;
export const IconListPlus = (p: P) => <Svg {...p}><path d="M4 6h11M4 11h11M4 16h7M17 14v6M14 17h6" /></Svg>;
export const IconTrash = (p: P) => <Svg {...p}><path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12" /></Svg>;
export const IconMore = (p: P) => <Svg {...p}><circle cx="6" cy="12" r="1.4" fill="currentColor" /><circle cx="12" cy="12" r="1.4" fill="currentColor" /><circle cx="18" cy="12" r="1.4" fill="currentColor" /></Svg>;
export const IconShare = (p: P) => <Svg {...p}><path d="M12 15V4M8 8l4-4 4 4" /><path d="M5 13v5a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-5" /></Svg>;
export const IconImage = (p: P) => <Svg {...p}><rect x="4" y="5" width="16" height="14" rx="2" /><circle cx="9" cy="10" r="1.6" /><path d="M20 16l-5-5-8 8" /></Svg>;
export const IconPencil = (p: P) => <Svg {...p}><path d="M4 20l4-1 11-11-3-3L5 16z" /></Svg>;
export const IconGrip = (p: P) => <Svg {...p}><circle cx="9" cy="7" r="1.2" fill="currentColor" /><circle cx="15" cy="7" r="1.2" fill="currentColor" /><circle cx="9" cy="12" r="1.2" fill="currentColor" /><circle cx="15" cy="12" r="1.2" fill="currentColor" /><circle cx="9" cy="17" r="1.2" fill="currentColor" /><circle cx="15" cy="17" r="1.2" fill="currentColor" /></Svg>;
export const IconCode = (p: P) => <Svg {...p}><path d="M8 8l-4 4 4 4M16 8l4 4-4 4" /></Svg>;
