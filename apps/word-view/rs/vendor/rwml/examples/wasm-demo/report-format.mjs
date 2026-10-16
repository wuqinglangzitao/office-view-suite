function featureItem(item) {
  if (item && typeof item === "object") {
    if (typeof item.path === "string") {
      const format = item.format || "metafile";
      const bytes = Number(item.bytes) || 0;
      return `${format} ${item.path} (${bytes} bytes)`;
    }
    const label = item.kind ?? item.reason;
    if (label !== undefined && item.count !== undefined) {
      return `${label}=${item.count}`;
    }
    return JSON.stringify(item);
  }
  return String(item);
}

export function featureEntries(features) {
  return Object.entries(features || {})
    .filter(([, value]) => {
      if (Array.isArray(value)) {
        return value.length > 0;
      }
      return Number(value) > 0;
    })
    .map(([key, value]) => {
      if (Array.isArray(value)) {
        return `${key}: ${value.map(featureItem).join(", ")}`;
      }
      return `${key}: ${value}`;
    });
}

export function warningEntries(warnings) {
  return (warnings || []).map((warning) => {
    if (warning?.kind) {
      return `${warning.kind}: ${JSON.stringify(warning)}`;
    }
    return JSON.stringify(warning);
  });
}
