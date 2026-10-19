import { crc32 } from 'node:zlib';

// A complete, deterministic OOXML workbook. Values below are independent parser
// expectations: rich A1, cached SUM(1,2)=3 in A2, and a 100x50 image anchored at B2.
export function createCompatibilityWorkbook() {
  const main = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main';
  const rel = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
  const packageRel = 'http://schemas.openxmlformats.org/package/2006/relationships';
  const files = {
    '[Content_Types].xml': `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
      <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
      <Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>
      <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
      <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
      <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
      <Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>
      </Types>`,
    '_rels/.rels': `<Relationships xmlns="${packageRel}"><Relationship Id="rId1" Type="${rel}/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
    'xl/workbook.xml': `<workbook xmlns="${main}" xmlns:r="${rel}"><sheets><sheet name="Compatibility" sheetId="1" r:id="rId1"/></sheets></workbook>`,
    'xl/_rels/workbook.xml.rels': `<Relationships xmlns="${packageRel}"><Relationship Id="rId1" Type="${rel}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="${rel}/sharedStrings" Target="sharedStrings.xml"/></Relationships>`,
    'xl/sharedStrings.xml': `<sst xmlns="${main}" count="1" uniqueCount="1"><si><r><rPr><b/><sz val="14"/><color rgb="FFFF0000"/></rPr><t>Bold</t></r><r><t xml:space="preserve"> plain</t></r></si></sst>`,
    'xl/worksheets/sheet1.xml': `<worksheet xmlns="${main}" xmlns:r="${rel}"><dimension ref="A1:B2"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15"/><sheetData>
      <row r="1"><c r="A1" t="s"><v>0</v></c></row><row r="2"><c r="A2"><f>SUM(1,2)</f><v>3</v></c></row>
      </sheetData><drawing r:id="rId1"/></worksheet>`,
    'xl/worksheets/_rels/sheet1.xml.rels': `<Relationships xmlns="${packageRel}"><Relationship Id="rId1" Type="${rel}/drawing" Target="../drawings/drawing1.xml"/></Relationships>`,
    'xl/drawings/drawing1.xml': `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
      <xdr:oneCellAnchor><xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx="952500" cy="476250"/>
      <xdr:pic><xdr:nvPicPr><xdr:cNvPr id="1" name="Compatibility image"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip xmlns:r="${rel}" r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>`,
    'xl/drawings/_rels/drawing1.xml.rels': `<Relationships xmlns="${packageRel}"><Relationship Id="rId1" Type="${rel}/image" Target="../media/image1.png"/></Relationships>`,
    'xl/media/image1.png': Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/lF8AAAAASUVORK5CYII=', 'base64'),
  };
  const local = [], directory = [];
  let offset = 0;
  for (const [path, value] of Object.entries(files)) {
    const name = Buffer.from(path), data = Buffer.isBuffer(value) ? value : Buffer.from(value);
    const checksum = crc32(data);
    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50); header.writeUInt16LE(20, 4);
    header.writeUInt32LE(checksum, 14); header.writeUInt32LE(data.length, 18); header.writeUInt32LE(data.length, 22); header.writeUInt16LE(name.length, 26);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(0x02014b50); entry.writeUInt16LE(20, 4); entry.writeUInt16LE(20, 6);
    entry.writeUInt32LE(checksum, 16); entry.writeUInt32LE(data.length, 20); entry.writeUInt32LE(data.length, 24); entry.writeUInt16LE(name.length, 28); entry.writeUInt32LE(offset, 42);
    local.push(header, name, data); directory.push(entry, name);
    offset += header.length + name.length + data.length;
  }
  const central = Buffer.concat(directory), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50); end.writeUInt16LE(directory.length / 2, 8); end.writeUInt16LE(directory.length / 2, 10);
  end.writeUInt32LE(central.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, central, end]);
}
