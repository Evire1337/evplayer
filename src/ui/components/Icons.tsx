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
export const IconChart = (p: P) => <Svg {...p}><path d="M4 20h16" /><path d="M7 16v-5M12 16V6M17 16v-8" /></Svg>;
export const IconFolder = (p: P) => <Svg {...p}><path d="M4 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z" /></Svg>;
export const IconLink = (p: P) => <Svg {...p}><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" /></Svg>;
export const IconRadio = (p: P) => <Svg {...p}><circle cx="12" cy="12" r="2" /><path d="M7.8 7.8a6 6 0 0 0 0 8.4M16.2 7.8a6 6 0 0 1 0 8.4M5 5a10 10 0 0 0 0 14M19 5a10 10 0 0 1 0 14" /></Svg>;
export const IconUser = (p: P) => <Svg {...p}><circle cx="12" cy="8.5" r="3.5" /><path d="M5 20a7 7 0 0 1 14 0" /></Svg>;
export const IconSettings = (p: P) => <Svg {...p}><circle cx="12" cy="12" r="3" /><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1" /></Svg>;
export const IconDownload = (p: P) => <Svg {...p}><path d="M12 4v11M8 11l4 4 4-4" /><path d="M5 19h14" /></Svg>;
export const IconBack = (p: P) => <Svg {...p}><path d="M15 5l-7 7 7 7" /></Svg>;
export const IconSparkle = (p: P) => <Svg {...p}><path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z" /><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z" /></Svg>;
export const IconMusic = (p: P) => <Svg {...p}><path d="M9 18V6l10-2v12" /><circle cx="6.5" cy="18" r="2.5" /><circle cx="16.5" cy="16" r="2.5" /></Svg>;
export const IconPlayCircle = (p: P) => <Svg {...p}><circle cx="12" cy="12" r="8.5" /><path d="M10 8.8v6.4l5.2-3.2z" fill="currentColor" stroke="none" /></Svg>;
export const IconEq = (p: P) => <Svg {...p}><path d="M6 4v16M12 4v16M18 4v16" /><rect x="4" y="13" width="4" height="3" rx="1" fill="currentColor" /><rect x="10" y="7" width="4" height="3" rx="1" fill="currentColor" /><rect x="16" y="11" width="4" height="3" rx="1" fill="currentColor" /></Svg>;
export const IconPalette = (p: P) => <Svg {...p}><path d="M12 3.5a8.5 8.5 0 1 0 0 17c1.2 0 1.8-.8 1.8-1.7 0-1.2-1-1.5-1-2.6 0-1 .8-1.7 1.8-1.7h2a3.9 3.9 0 0 0 3.9-3.9C20.5 6.7 16.7 3.5 12 3.5z" /><circle cx="8" cy="10" r="1.1" fill="currentColor" /><circle cx="11.5" cy="7.3" r="1.1" fill="currentColor" /><circle cx="15.5" cy="8.6" r="1.1" fill="currentColor" /></Svg>;
export const IconKeyboard = (p: P) => <Svg {...p}><rect x="3" y="6" width="18" height="12" rx="2.5" /><path d="M7 10h.01M10 10h.01M13 10h.01M16 10h.01M7 14h10" /></Svg>;
export const IconPip = (p: P) => <Svg {...p}><rect x="3" y="5" width="18" height="14" rx="2.5" /><rect x="11.5" y="11.5" width="7" height="5" rx="1.2" fill="currentColor" stroke="none" /></Svg>;
export const IconPlug = (p: P) => <Svg {...p}><path d="M9 3v5M15 3v5M6.5 8h11v3a5.5 5.5 0 0 1-11 0z" /><path d="M12 16.5V21" /></Svg>;
export const IconChat = (p: P) => <Svg {...p}><path d="M5 5h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-7l-4.5 3.5V17H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z" /><circle cx="9" cy="11" r="1" fill="currentColor" /><circle cx="15" cy="11" r="1" fill="currentColor" /></Svg>;
export const IconDatabase = (p: P) => <Svg {...p}><ellipse cx="12" cy="6" rx="7" ry="2.8" /><path d="M5 6v12c0 1.5 3.1 2.8 7 2.8s7-1.3 7-2.8V6M5 12c0 1.5 3.1 2.8 7 2.8s7-1.3 7-2.8" /></Svg>;
export const IconInfo = (p: P) => <Svg {...p}><circle cx="12" cy="12" r="8.5" /><path d="M12 11v5M12 8h.01" /></Svg>;
export const IconCheck = (p: P) => <Svg {...p}><path d="M5 12.5l4.5 4.5L19 7.5" /></Svg>;
export const IconRefresh = (p: P) => <Svg {...p}><path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3" /><path d="M19.5 4.5v4h-4" /></Svg>;
export const IconUndo = (p: P) => <Svg {...p}><path d="M9 7L5 11l4 4" /><path d="M5 11h9a5 5 0 0 1 0 10h-2" /></Svg>;
export const IconSort = (p: P) => <Svg {...p}><path d="M7 4v16M4 17l3 3 3-3" /><path d="M13 6h8M13 11h6M13 16h3" /></Svg>;
export const IconGridView = (p: P) => <Svg {...p}><rect x="4" y="4" width="7" height="7" rx="1.5" /><rect x="13" y="4" width="7" height="7" rx="1.5" /><rect x="4" y="13" width="7" height="7" rx="1.5" /><rect x="13" y="13" width="7" height="7" rx="1.5" /></Svg>;
export const IconListView = (p: P) => <Svg {...p}><rect x="4" y="5" width="4" height="4" rx="1" /><rect x="4" y="15" width="4" height="4" rx="1" /><path d="M11 7h9M11 17h9" /></Svg>;
