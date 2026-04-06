'use strict';

require('dotenv').config();

const express = require('express');
const auth = require('./middleware/auth');
const healthRouter = require('./routes/health');
const proxyRouter = require('./routes/proxy');

function createApp() {
  const app = express();

  // Parse JSON bodies (up to 10 MB to support large prompt payloads).
  app.use(express.json({ limit: '10mb' }));

  // Health check — no auth required.
  app.use(healthRouter);

  // Apply optional gateway authentication to all other routes.
  app.use(auth);

  // Proxy routes.
  app.use(proxyRouter);

  // 404 fallback.
  app.use((_req, res) => {
    res.status(404).json({
      error: {
        message: 'Route not found.',
        type: 'not_found',
        code: 'route_not_found',
      },
    });
  });

  return app;
}

module.exports = createApp;
