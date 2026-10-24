<script lang="ts">
  /** Small stroke icon set, drawn on a 24x24 grid. No external dependency. */
  export type IconName =
    | 'mixer'
    | 'apps'
    | 'eq'
    | 'record'
    | 'routing'
    | 'interface'
    | 'system'
    | 'mic'
    | 'wave'
    | 'speaker'
    | 'headphones'
    | 'plus'
    | 'trash'
    | 'save'
    | 'chevron'
    | 'check'
    | 'stop'
    | 'folder'
    | 'bolt'
    | 'palette'
    | 'alert';

  interface Props {
    name: IconName;
    size?: number;
    stroke?: number;
  }
  let { name, size = 18, stroke = 1.7 }: Props = $props();

  const paths: Record<IconName, string> = {
    mixer: 'M6 4v16M12 4v16M18 4v16M4 9h4M10 15h4M16 8h4',
    apps: 'M4 5h6v6H4zM14 5h6v6h-6zM4 15h6v4H4zM14 15h6v4h-6z',
    eq: 'M3 17c3 0 3-10 6-10s3 12 6 12 3-8 6-8',
    record: 'M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10z',
    routing: 'M5 7h4a4 4 0 0 1 4 4v2a4 4 0 0 0 4 4h2M5 17h4M17 7h2M19 7l-2-2M19 7l-2 2M19 17l-2-2M19 17l-2 2',
    interface: 'M3 9h18v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM7 13h.01M11 13h6M7 5v4M17 5v4',
    system: 'M3 12h4l3-7 4 14 3-7h4',
    mic: 'M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3zM5 11a7 7 0 0 0 14 0M12 18v3',
    wave: 'M3 12c2-6 4-6 6 0s4 6 6 0 4-6 6 0',
    speaker: 'M4 10v4h4l5 4V6l-5 4zM16.5 9a4.5 4.5 0 0 1 0 6M19 6.5a8 8 0 0 1 0 11',
    headphones: 'M4 15v-3a8 8 0 0 1 16 0v3M4 15h3v5H5a1 1 0 0 1-1-1zM20 15h-3v5h2a1 1 0 0 0 1-1z',
    plus: 'M12 5v14M5 12h14',
    trash: 'M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12M10 11v5M14 11v5',
    save: 'M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6',
    chevron: 'M6 9l6 6 6-6',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    stop: 'M7 7h10v10H7z',
    folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
    bolt: 'M13 3L5 13h6l-1 8 8-10h-6z',
    palette: 'M12 3a9 9 0 1 0 0 18c1.2 0 2-.8 2-1.8 0-.5-.2-.9-.5-1.3-.3-.4-.5-.8-.5-1.3 0-1 .8-1.8 1.8-1.8H17a4 4 0 0 0 4-4C21 6.5 17 3 12 3zM7.5 11h.01M10 7.5h.01M14.5 7.5h.01',
    alert: 'M12 4l9 16H3zM12 10v4M12 17h.01',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <path d={paths[name]} />
</svg>
