import { useJam } from "./store";
import { letIn, turnDown } from "./invites";

/** friends who pressed "Ask to join" in Discord, each with a yes and a no */
export default function JamAsks() {
  const asks = useJam((s) => s.asks);
  if (!asks.length) return null;
  return (
    <div className="jam-asks">
      {asks.map((user) => (
        <div key={user.id} className="jam-ask glass" role="alert">
          {user.avatar ? <img className="jam-avatar" src={user.avatar} alt="" /> : <span className="jam-avatar" style={{ background: "#5865f2" }}>{(user.globalName ?? user.username).slice(0, 1).toUpperCase()}</span>}
          <span className="jam-ask-text">
            <b>{user.globalName ?? user.username}</b> хочет слушать вместе с вами
          </span>
          <button type="button" className="btn primary" onClick={() => void letIn(user)}>Пустить</button>
          <button type="button" className="btn ghost" onClick={() => turnDown(user)}>Нет</button>
        </div>
      ))}
    </div>
  );
}
