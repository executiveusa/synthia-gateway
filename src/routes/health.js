'use strict';

const router = require('express').Router();

router.get('/health', (_req, res) => {
  res.json({
    status: 'ok',
    service: 'synthia-gateway',
    version: require('../../package.json').version,
    timestamp: new Date().toISOString(),
  });
});

module.exports = router;
