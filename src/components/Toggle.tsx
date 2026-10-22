export default function Toggle({ label, description, checked, onChange }: { label: string; description: string; checked: boolean; onChange: (value: boolean) => void }) {
  return <label className="toggle-row"><span><b>{label}</b><small>{description}</small></span><input type="checkbox" role="switch" checked={checked} onChange={e => onChange(e.target.checked)}/><span className="switch" aria-hidden="true"/></label>;
}
