// Native Node.js addon loader for rip-receipt.
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const native = require('./rip_nodejs.node');
export const { renderImage, renderRaster, renderEscpos, renderHtml, renderText } = native;
