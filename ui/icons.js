// Profile icons: one line-icon set (24 px grid, 1.8 stroke, round caps), shared by the popup and settings.
// A profile stores the icon's name; an empty or unknown name shows the accent dot instead.
const HOPPY_ICONS = {
  sparkle: '<path d="M11 3.5l1.9 5.1 5.1 1.9-5.1 1.9-1.9 5.1-1.9-5.1L4 10.5l5.1-1.9z"/><path d="M18.5 15v5M16 17.5h5"/>',
  pen: '<path d="M4 20h4L19 9a2.1 2.1 0 0 0-3-3L5 17v3z"/><path d="M14.5 7.5l3 3"/>',
  globe: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17"/><path d="M12 3.5c2.3 2.4 3.5 5.2 3.5 8.5s-1.2 6.1-3.5 8.5c-2.3-2.4-3.5-5.2-3.5-8.5s1.2-6.1 3.5-8.5z"/>',
  chat: '<path d="M5.5 4.5h13a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H12l-4.5 3.5v-3.5h-2a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2z"/>',
  code: '<path d="M8.5 8 4.5 12l4 4M15.5 8l4 4-4 4M13.5 5.5l-3 13"/>',
  terminal: '<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M7 9.5l3 2.5-3 2.5M12.5 15H17"/>',
  book: '<path d="M5 5a2 2 0 0 1 2-2h12v14H7a2 2 0 0 0-2 2z"/><path d="M5 19a2 2 0 0 0 2 2h12v-4"/>',
  mail: '<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M3.5 7.5 12 13l8.5-5.5"/>',
  doc: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/>',
  list: '<path d="M9.5 6.5H20M9.5 12H20M9.5 17.5H20"/><circle cx="5" cy="6.5" r="1.1" fill="currentColor" stroke="none"/><circle cx="5" cy="12" r="1.1" fill="currentColor" stroke="none"/><circle cx="5" cy="17.5" r="1.1" fill="currentColor" stroke="none"/>',
  bulb: '<path d="M9.5 18h5M10.5 21h3"/><path d="M12 3a6 6 0 0 0-3.6 10.8c.7.5 1.1 1.3 1.1 2.1v.1h5v-.1c0-.8.4-1.6 1.1-2.1A6 6 0 0 0 12 3z"/>',
  search: '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>',
  briefcase: '<rect x="3" y="7" width="18" height="13" rx="2.5"/><path d="M8.5 7V5.5A1.5 1.5 0 0 1 10 4h4a1.5 1.5 0 0 1 1.5 1.5V7M3 12.5h18"/>',
  calendar: '<rect x="3.5" y="5" width="17" height="15.5" rx="2.5"/><path d="M3.5 10h17M8 3v4M16 3v4"/>',
  heart: '<path d="M12 20s-8-4.7-8-10.2A4.3 4.3 0 0 1 12 7.4a4.3 4.3 0 0 1 8 2.4C20 15.3 12 20 12 20z"/>',
  star: '<path d="M12 3.5l2.6 5.4 5.9.8-4.3 4.1 1 5.8L12 16.8l-5.2 2.8 1-5.8-4.3-4.1 5.9-.8z"/>',
  flag: '<path d="M5.5 21V4M5.5 4.5h11.5l-2.2 4 2.2 4H5.5"/>',
  music: '<path d="M9 18V5.5l11-2V16"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="17.5" cy="16" r="2.5"/>',
  camera: '<path d="M4.5 7.5h3l1.8-2.5h5.4l1.8 2.5h3a1.5 1.5 0 0 1 1.5 1.5v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5V9a1.5 1.5 0 0 1 1.5-1.5z"/><circle cx="12" cy="13.5" r="3.5"/>',
  leaf: '<path d="M20 4C10.5 4 5 9 5 15.5V19h3.5C15 19 20 13.5 20 4z"/><path d="M4 20l9.5-9.5"/>',
  coffee: '<path d="M4 9h13v5a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5z"/><path d="M17 10.5h1.5a2.5 2.5 0 0 1 0 5H17M8.5 3.5V6M12.5 3.5V6"/>',
  bug: '<rect x="7" y="7.5" width="10" height="13" rx="5"/><path d="M12 12v8.5M7 13.5H3.5M20.5 13.5H17M7 17.5l-3 1.5M17 17.5l3 1.5M8 9.5 5.5 7M16 9.5 18.5 7M9.5 7.5a2.5 2.5 0 0 1 5 0"/>',
  home: '<path d="M4 11.5 12 4.5l8 7"/><path d="M6.5 10v10h11V10M10 20v-5h4v5"/>',
  user: '<circle cx="12" cy="8.5" r="4"/><path d="M4.5 20.5a7.5 7.5 0 0 1 15 0"/>',
  bolt: '<path d="M13 3 5 13.5h6L10.5 21 19 10.5h-6z"/>',
  moon: '<path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z"/>',
  wand: '<path d="M4 20 14.5 9.5M16.5 3.5v3M21 8h-3M19.5 4.5l-2 2M9.5 4v2M4 9.5h2"/>',
  hash: '<path d="M5 9h14M5 15h14M10.5 4 8.5 20M15.5 4l-2 16"/>',
};

// <svg> for an icon name, or null when the profile has none.
function profileIcon(name, cls = "picon-svg") {
  const body = HOPPY_ICONS[name];
  if (!body) return null;
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", cls);
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.8");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.innerHTML = body;
  return svg;
}

// The profile's mark: its icon, or the accent dot.
function profileMark(name) {
  const svg = profileIcon(name);
  if (svg) return svg;
  const dot = document.createElement("span");
  dot.className = "dot";
  return dot;
}
