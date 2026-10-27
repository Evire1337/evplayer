import logo from "@/logo.svg";
import { useNavigation, type Page } from "../navigation";
import { IconDisc, IconFolder, IconHome, IconLibrary, IconSearch, IconSettings } from "./Icons";

const ITEMS: { page: Page; label: string; icon: (p: { size?: number }) => React.ReactNode }[] = [
  { page: "now", label: "Сейчас играет", icon: IconDisc },
  { page: "home", label: "Главная", icon: IconHome },
  { page: "search", label: "Поиск", icon: IconSearch },
  { page: "library", label: "Библиотека", icon: IconLibrary },
  { page: "files", label: "Мои файлы", icon: IconFolder },
];

// the floating column of sections on the left
export default function Rail() {
  const { page, go } = useNavigation();
  return (
    <nav className="rail glass" aria-label="Разделы">
      <img src={logo} alt="EvPlayer" className="rail-logo" />
      {ITEMS.map(({ page: id, label, icon: Icon }) => (
        <button
          key={id}
          type="button"
          className={`rail-btn${page === id || (id === "library" && (page === "playlist" || page === "smart")) ? " is-active" : ""}`}
          aria-label={label}
          aria-current={page === id ? "page" : undefined}
          onClick={() => go(id)}
        >
          <Icon size={21} />
          <span className="rail-tip">{label}</span>
        </button>
      ))}
      <span className="rail-gap" />
      <button
        type="button"
        className={`rail-btn${page === "settings" ? " is-active" : ""}`}
        aria-label="Настройки"
        aria-current={page === "settings" ? "page" : undefined}
        onClick={() => go("settings")}
      >
        <IconSettings size={21} />
        <span className="rail-tip">Настройки</span>
      </button>
    </nav>
  );
}
