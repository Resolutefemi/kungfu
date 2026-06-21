// Express.js hello-world server, for direct comparison with kungfu's bench.
//
//   npm install
//   npm start
//   oha -z 5s -c 64 http://localhost:3002/hello

const express = require('express');
const app = express();

app.get('/hello', (req, res) => {
  res.json({ message: 'world' });
});

app.listen(3002, '127.0.0.1', () => {
  console.log('express bench listening on http://127.0.0.1:3002');
});
