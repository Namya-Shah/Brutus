"use client";
import { useEffect, useState, type FormEvent } from "react";
import { ArrowUpRight, CheckCircle2, LoaderCircle, ShieldCheck } from "lucide-react";
const storageKey = "brutus-waitlist-signup";
type Signup = { email: string; removalToken?: string };

export default function Waitlist() {
  const [email, setEmail] = useState("");
  const [platform, setPlatform] = useState("");
  const [signup, setSignup] = useState<Signup | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [privacyOpen, setPrivacyOpen] = useState(false);
  useEffect(() => { try { const saved = localStorage.getItem(storageKey); if (saved) setSignup(JSON.parse(saved)); } catch { /* Signup works without browser storage. */ } }, []);
  async function join(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setBusy(true); setError("");
    const form = new FormData(event.currentTarget);
    try {
      const response = await fetch("/api/waitlist", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ email, platform, website: form.get("website") }) });
      const data = await response.json() as { error?: string; removalToken?: string };
      if (!response.ok) throw new Error(data.error || "We couldn’t save your signup. Please try again.");
      const saved = { email: email.trim().toLowerCase(), removalToken: data.removalToken };
      setSignup(saved);
      try { localStorage.setItem(storageKey, JSON.stringify(saved)); } catch { /* Storage is optional. */ }
    } catch (err) { setError(err instanceof Error ? err.message : "Connection interrupted. Please try again."); }
    finally { setBusy(false); }
  }
  async function remove() {
    if (!signup?.removalToken) return;
    setBusy(true); setError("");
    try {
      const response = await fetch("/api/waitlist", { method: "DELETE", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ token: signup.removalToken }) });
      if (!response.ok) throw new Error("Couldn’t remove your signup. Please try again.");
      setSignup(null); setEmail("");
      try { localStorage.removeItem(storageKey); } catch { /* Storage is optional. */ }
    } catch (err) { setError(err instanceof Error ? err.message : "Please try again."); }
    finally { setBusy(false); }
  }
  return <div className="waitlist-form-wrap">
    {signup ? <div className="signup-success" role="status"><span className="success-icon"><CheckCircle2 size={24} /></span><h3>You’re on the list.</h3><p>We’ve saved <strong>{signup.email}</strong>.<br />We’ll be in touch when early access opens.</p>{signup.removalToken && <button className="remove-signup" disabled={busy} onClick={remove}>{busy ? "Removing…" : "Remove my signup"}</button>}</div> : <form onSubmit={join} className="waitlist-form"><label htmlFor="email">Your email address</label><input id="email" name="email" type="email" autoComplete="email" placeholder="you@company.com" value={email} onChange={e => setEmail(e.target.value)} maxLength={254} required aria-describedby={error ? "signup-error" : "signup-privacy"} /><label htmlFor="platform">What do you build on? <span>Optional</span></label><select id="platform" name="platform" value={platform} onChange={e => setPlatform(e.target.value)}><option value="">Select your platform</option><option value="macos">macOS</option><option value="windows">Windows</option><option value="linux">Linux</option><option value="multiple">A little of everything</option></select><div className="honeypot" aria-hidden="true"><label htmlFor="website">Website</label><input id="website" name="website" type="text" tabIndex={-1} autoComplete="off" /></div><button className="button button-primary" disabled={busy} type="submit">{busy ? <><LoaderCircle size={16} />Joining…</> : <>Join the waitlist<ArrowUpRight size={17} /></>}</button><p className="signup-privacy" id="signup-privacy"><ShieldCheck size={13} />Only Brutus access and launch updates.</p></form>}
    {error && <p id="signup-error" className="form-error" role="alert">{error}</p>}
    <button className="privacy-toggle" aria-expanded={privacyOpen} onClick={() => setPrivacyOpen(!privacyOpen)}>How we use your information</button>
    {privacyOpen && <p className="privacy-copy">We store your email and optional platform preference to manage Brutus early access and send launch updates. We don’t sell your information or use advertising trackers. Keep this browser’s confirmation to remove your entry at any time. Clearing your browser’s site data removes your saved removal link.</p>}
  </div>;
}
