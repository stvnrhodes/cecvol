import { peerSocket } from "messaging";
import document from "document";

document.getElementById("off").addEventListener("click", (evt) => { onOff(false); });
document.getElementById("on").addEventListener("click", (evt) => { onOff(true); });
document.getElementById("input1").addEventListener("click", (evt) => { setInput('1'); });
document.getElementById("input2").addEventListener("click", (evt) => { setInput('2'); });
document.getElementById("input3").addEventListener("click", (evt) => { setInput('3'); });
document.getElementById("input4").addEventListener("click", (evt) => { setInput('4'); });
document.getElementById("volup").addEventListener("click", (evt) => { sendVol(1); });
document.getElementById("voldown").addEventListener("click", (evt) => { sendVol(-1); });
document.getElementById("wol").addEventListener("click", wol);

// Shorthand functions for generating data, copied from cec.stevenandbonnie.com javascript
function sendVol(level) {
    executeCommand({
        'command': 'action.devices.commands.volumeRelative',
        'params': {
            'relativeSteps': level
        }
    })
}
function wol() {
    executeCommand({
        'command': 'action.devices.commands.wol',
    })
}
function setInput(newInput) {
    executeCommand({
        'command': 'action.devices.commands.SetInput',
        'params': {
            'newInput': newInput
        }
    })
}
function onOff(on) {
    executeCommand({
        'command': 'action.devices.commands.OnOff',
        'params': {
            'on': on
        }
    })
}


function executeCommand(data) {
  if (peerSocket.readyState === peerSocket.OPEN) {
    // Send the data to peer as a message
    peerSocket.send(data);
    status.text = ""
  } else {
    console.log("could not send " + JSON.stringify(data));
    status.text = "socket closed";
  }
}

// Add/remove statuses as things happen
let status = document.getElementById("status");
peerSocket.addEventListener("open", (evt) => { status.text = "" });
peerSocket.addEventListener("close", (evt) => { status.text = evt.reason });
peerSocket.addEventListener("error", (err) => { status.text = err.message });
peerSocket.addEventListener("message", (evt) => { status.text = evt.data });