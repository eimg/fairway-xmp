import type { ReactNode } from 'react';
import { ArrowUpRight } from 'lucide-react';
import { invoke, isDesktop } from './bridge';

export default function GuideLink({ section, children, className = 'secondary', title, onOpen, onError }: {
  section: 'getting-started' | 'cli';
  children: ReactNode;
  className?: string;
  title?: string;
  onOpen?: () => void;
  onError: (message: string) => void;
}) {
  return <a className={className} href={`https://eimg.github.io/fairway-xmp/#${section}`} target="_blank" rel="noreferrer" title={title} onClick={event => {
    onOpen?.();
    if (!isDesktop) return;
    event.preventDefault();
    void invoke<void>('open_target', { target: `guide:${section}` }).catch(error => {
      onError(`Could not open the guide: ${error instanceof Error ? error.message : String(error)}`);
    });
  }}>{children}<ArrowUpRight size={14} aria-hidden="true" /></a>;
}
