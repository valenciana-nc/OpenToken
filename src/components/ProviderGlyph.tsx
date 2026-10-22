import { providerDefinition, type ProviderId } from '../providers';

/** Bundled reference provider marks (MIT); alpha masks follow the selected panel theme. */
export default function ProviderGlyph({ provider, size = 23 }: { provider: ProviderId; size?: number }) {
  const mark = `url("${providerDefinition(provider).mark}")`;
  return <span className={`provider-mark provider-mark-${provider}`} aria-hidden="true" style={{ width: size, height: size, maskImage: mark, WebkitMaskImage: mark }}/>;
}
