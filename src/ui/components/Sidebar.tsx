import logo from "@/logo.svg";
import { useNavigation, type Page } from "../navigation";
import { IconHome, IconLibrary, IconSearch } from "./Icons";

const ITEMS: { page: Page; label: string; icon: (p: { size?: number }) => React.ReactNode }[] = [
  { page: "home", label: "Главная", icon: IconHome },
  { page: "search", label: "Поиск", icon: IconSearch },
  { page: "library", label: "Библиотека", icon: IconLibrary },
];

export default function Sidebar() {
  const { page, go } = useNavigation();
  return (
    <nav className="sidebar glass" aria-label="Разделы">
      <div className="brand">
        <img src={logo} alt="" className="brand-logo" />
        <span className="brand-name">EvPlayer</span>
      </div>
      <ul className="nav-list">
        {ITEMS.map(({ page: id, label, icon: Icon }) => (
          <li key={id}>
            <button type="button" className={`nav-item${page === id ? " is-active" : ""}`} onClick={() => go(id)} aria-current={page === id ? "page" : undefined}>
              <Icon size={19} />
              <span>{label}</span>
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
