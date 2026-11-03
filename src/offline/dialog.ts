import { create } from "zustand";

// which lists the "save for offline" dialog starts with ticked; null = closed
export const useDownloadDialog = create<{ open: boolean; preset: string[] }>(() => ({ open: false, preset: [] }));

export const openDownloadDialog = (preset: string[] = []) => useDownloadDialog.setState({ open: true, preset });
export const closeDownloadDialog = () => useDownloadDialog.setState({ open: false });
