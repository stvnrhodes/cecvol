import { peerSocket } from "messaging";

// It'd be a good idea to make the bearer token fetched dynamically
// and to put the password in settings, but that takes more work.
// For now, we've manually generated a bearer token valid until 2121.

peerSocket.addEventListener("message", (e) => {
  let cmd = e.data;
  // Copied from cec.stevenandbonnie.com javascript
  fetch('https://cec.stevenandbonnie.com/fulfillment', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': 'Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJjZWMuc3RldmVuYW5kYm9ubmllLmNvbSIsImV4cCI6NDc3ODQ1MjQ3MCwibmJmIjoxNjI0ODUyNDcwLCJpYXQiOjE2MjQ4NTI0NzB9.8DKqpmMDUMlkVMTPMgSgfXACDX1gQBs83K36vwHvzOg'
    },
    body: JSON.stringify({
      'requestId': 'no-id',
      'inputs': [{
        'intent': 'action.devices.EXECUTE',
        'payload': {
          'commands': [{
            'devices': [{
              'id': '123'
            }],
            'execution': [cmd]
          }]
        }
      }]
    })
  }).then(response => { 
    if (!response.ok) {
      peerSocket.send(response.statusText);
    }
  });
});