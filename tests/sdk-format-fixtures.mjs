// These containers intentionally omit compressed document bodies. They are for
// routing/lifecycle tests; successful parsing is supplied by the fake Worker.
export function zipDirectory(names, { zip64 = false } = {}) {
  const records = names.map(name => {
    const encoded = Buffer.from(name);
    const record = Buffer.alloc(46 + encoded.length);
    record.writeUInt32LE(0x02014b50);
    record.writeUInt16LE(encoded.length, 28);
    encoded.copy(record, 46);
    return record;
  });
  const directory = Buffer.concat(records);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(zip64 ? 0xffff : names.length, 8);
  end.writeUInt16LE(zip64 ? 0xffff : names.length, 10);
  end.writeUInt32LE(zip64 ? 0xffffffff : directory.length, 12);
  end.writeUInt32LE(zip64 ? 0xffffffff : 0, 16);
  if (!zip64) return Buffer.concat([directory, end]);
  const wideEnd = Buffer.alloc(56);
  wideEnd.writeUInt32LE(0x06064b50);
  wideEnd.writeBigUInt64LE(44n, 4);
  wideEnd.writeBigUInt64LE(BigInt(names.length), 24);
  wideEnd.writeBigUInt64LE(BigInt(names.length), 32);
  wideEnd.writeBigUInt64LE(BigInt(directory.length), 40);
  const locator = Buffer.alloc(20);
  locator.writeUInt32LE(0x07064b50);
  locator.writeBigUInt64LE(BigInt(directory.length), 8);
  locator.writeUInt32LE(1, 16);
  return Buffer.concat([directory, wideEnd, locator, end]);
}

export function formatBytes(format) {
  if (format === 'pdf') return new TextEncoder().encode('%PDF-1.7\n');
  if (format === 'doc') {
    const bytes = Buffer.alloc(512);
    bytes.set([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
    bytes.set(Buffer.from('WordDocument\0', 'utf16le'), 128);
    return bytes;
  }
  const names = {
    docx: ['word/document.xml'], docm: ['word/document.xml', 'word/vbaProject.bin'],
    xlsx: ['xl/workbook.xml'], xlsm: ['xl/workbook.xml', 'xl/vbaProject.bin'],
    pptx: ['ppt/presentation.xml'],
  };
  return zipDirectory(names[format]);
}
