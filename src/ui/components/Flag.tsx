// Flags drawn here: Windows shows country flag emoji as bare letters.

export function Flag({ code, size = 18 }: { code: "ru" | "gb" | "auto"; size?: number }) {
  const h = Math.round(size * 0.72);
  if (code === "auto") {
    return (
      <svg width={size} height={h} viewBox="0 0 24 17" aria-hidden="true">
        <rect width="24" height="17" rx="3" fill="none" stroke="currentColor" strokeWidth="1.4" />
        <circle cx="12" cy="8.5" r="4.2" fill="none" stroke="currentColor" strokeWidth="1.2" />
        <path d="M7.8 8.5h8.4M12 4.3c-2 2.2-2 6.2 0 8.4M12 4.3c2 2.2 2 6.2 0 8.4" fill="none" stroke="currentColor" strokeWidth="1" />
      </svg>
    );
  }
  return code === "ru" ? (
    <svg width={size} height={h} viewBox="0 0 24 17" aria-hidden="true">
      <clipPath id="flag-ru"><rect width="24" height="17" rx="3" /></clipPath>
      <g clipPath="url(#flag-ru)">
        <rect width="24" height="5.7" fill="#ffffff" />
        <rect y="5.7" width="24" height="5.7" fill="#1c57a5" />
        <rect y="11.4" width="24" height="5.7" fill="#d52b1e" />
      </g>
    </svg>
  ) : (
    <svg width={size} height={h} viewBox="0 0 24 17" aria-hidden="true">
      <clipPath id="flag-gb"><rect width="24" height="17" rx="3" /></clipPath>
      <g clipPath="url(#flag-gb)">
        <rect width="24" height="17" fill="#1b3a7a" />
        <path d="M0 0l24 17M24 0L0 17" stroke="#ffffff" strokeWidth="3.4" />
        <path d="M0 0l24 17M24 0L0 17" stroke="#cf142b" strokeWidth="1.2" />
        <path d="M12 0v17M0 8.5h24" stroke="#ffffff" strokeWidth="5.6" />
        <path d="M12 0v17M0 8.5h24" stroke="#cf142b" strokeWidth="3.2" />
      </g>
    </svg>
  );
}
