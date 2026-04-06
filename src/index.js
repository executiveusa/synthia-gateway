'use strict';

require('dotenv').config();

const createApp = require('./app');

const PORT = parseInt(process.env.PORT || '3000', 10);
const app = createApp();

const server = app.listen(PORT, () => {
  const mode = process.env.GATEWAY_API_KEY
    ? 'gateway-key (server-side API keys)'
    : 'pass-through (caller-supplied API keys)';

  console.log(`synthia-gateway listening on port ${PORT}`);
  console.log(`  mode : ${mode}`);
  console.log(`  env  : ${process.env.NODE_ENV || 'development'}`);
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('SIGTERM received, shutting down…');
  server.close(() => process.exit(0));
});

process.on('SIGINT', () => {
  console.log('SIGINT received, shutting down…');
  server.close(() => process.exit(0));
});

module.exports = server;
