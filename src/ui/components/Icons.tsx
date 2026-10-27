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
