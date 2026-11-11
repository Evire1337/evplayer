import { jam } from "./engine";
import { useJam } from "./store";
import { letIn, turnDown } from "./invites";

/**
 * Someone wants to listen along, each with a yes and a no: friends who pressed "Ask to
 * join" in Discord, and (for the host, while the session dialog is closed) people who came
 * with the code to a session that lets in on request.
 */
export default function JamAsks() {
  const asks = useJam((s) => s.asks);
  const allKnocks = useJam((s) => s.knocks);
  const dialogOpen = useJam((s) => s.open);
  // in the dialog they are right there
  const knocks = dialogOpen ? [] : allKnocks;
  if (!asks.length && !knocks.length) return null;
  return (
    <div className="jam-asks">
      {asks.map((user) => (
        <div key={user.id} className="jam-ask glass" role="alert">
          {user.avatar ? <img className="jam-avatar" src={user.avatar} alt="" /> : <span className="jam-avatar" style={{ background: "#5865f2" }}>{(user.globalName ?? user.username).slice(0, 1).toUpperCase()}</span>}
          <span className="jam-ask-text">
            <b>{user.globalName ?? user.username}</b> хочет слушать вместе с вами (Discord)
          </span>
          <button type="button" className="btn primary" onClick={() => void letIn(user)}>Пустить</button>
          <button type="button" className="btn ghost" onClick={() => turnDown(user)}>Нет</button>
        </div>
      ))}
      {knocks.map((k) => (
        <div key={k.id} className="jam-ask glass" role="alert">
          <span className="jam-avatar" style={{ background: k.color }}>{k.name.slice(0, 1).toUpperCase()}</span>
          <span className="jam-ask-text">
            <b>{k.name}</b> просится в сессию
          </span>
          <button type="button" className="btn primary" onClick={() => jam.admit(k.id)}>Пустить</button>
          <button type="button" className="btn ghost" onClick={() => jam.turnAway(k.id)}>Нет</button>
        </div>
      ))}
    </div>
  );
}