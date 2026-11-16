import { useLayoutEffect, useRef, type TextareaHTMLAttributes } from 'react';

// Textfeld, das mit seinem Inhalt wächst; die Obergrenze setzt das CSS über max-height (darüber scrollt es)
export function AutoHeightTextarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const ref = useRef<HTMLTextAreaElement>(null);

  // Höhe an den Inhalt anpassen, nach jeder Änderung und wenn sich die Breite ändert (Umbruch)
  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const fit = () => {
      node.style.height = 'auto';
      const borders = node.offsetHeight - node.clientHeight;
      node.style.height = `${node.scrollHeight + borders}px`;
    };
    fit();
    if (typeof ResizeObserver === 'undefined') return;
    let width = node.clientWidth;
    const observer = new ResizeObserver(() => {
      if (node.clientWidth === width) return;
      width = node.clientWidth;
      fit();
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [props.value]);

  return <textarea ref={ref} {...props} />;
}
