import { useState } from "react";

/** Small name field with a save button that opens after a click. */
export function NameForm({ label, placeholder, onSave }: { label: string; placeholder: string; onSave: (name: string) => void }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  if (!open) {
    return (
      <button className="btn btn-small btn-dashed-inline" onClick={() => setOpen(true)}>
        + {label}
      </button>
    );
  }
  const submit = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    onSave(trimmed);
    setName("");
    setOpen(false);
  };
  return (
    <form
      className="name-form"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <input autoFocus value={name} placeholder={placeholder} onChange={(e) => setName(e.target.value)} />
      <button className="btn btn-small btn-accent" type="submit" disabled={!name.trim()}>
        Save
      </button>
      <button className="btn btn-small btn-ghost" type="button" onClick={() => setOpen(false)}>
        Cancel
      </button>
    </form>
  );
}
