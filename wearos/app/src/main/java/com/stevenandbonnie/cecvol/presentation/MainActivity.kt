/* While this template provides a good starting point for using Wear Compose, you can always
 * take a look at https://github.com/android/wear-os-samples/tree/main/ComposeStarter and
 * https://github.com/android/wear-os-samples/tree/main/ComposeAdvanced to find the most up to date
 * changes to the libraries and their usages.
 */

package com.stevenandbonnie.cecvol.presentation

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Power
import androidx.compose.material.icons.rounded.PowerOff
import androidx.compose.material.icons.rounded.Transcribe
import androidx.compose.material.icons.rounded.VolumeDown
import androidx.compose.material.icons.rounded.VolumeOff
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Devices
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.ButtonDefaults
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.Icon
import androidx.wear.compose.material.MaterialTheme
import androidx.wear.compose.material.PositionIndicator
import androidx.wear.compose.material.Scaffold
import androidx.wear.compose.material.ScalingLazyColumn
import androidx.wear.compose.material.Text
import androidx.wear.compose.material.TimeText
import androidx.wear.compose.material.Vignette
import androidx.wear.compose.material.VignettePosition
import androidx.wear.compose.material.rememberScalingLazyListState
import androidx.wear.compose.material.scrollAway
import com.google.gson.Gson
import com.stevenandbonnie.cecvol.R
import com.stevenandbonnie.cecvol.presentation.theme.CECVolTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

class MainActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    installSplashScreen()

    super.onCreate(savedInstanceState)

    setTheme(android.R.style.Theme_DeviceDefault)

    setContent { WearApp() }
  }
}

@Composable
fun WearApp() {
  CECVolTheme {
    val listState = rememberScalingLazyListState()
    Scaffold(
      timeText = { TimeText(modifier = Modifier.scrollAway(listState)) },
      vignette = { Vignette(vignettePosition = VignettePosition.TopAndBottom) },
      positionIndicator = { PositionIndicator(scalingLazyListState = listState) }
    ) {
      val contentModifier = Modifier
        .fillMaxWidth()
        .padding(bottom = 8.dp)
      val iconModifier = Modifier
        .size(24.dp)
        .wrapContentSize(align = Alignment.Center)
      ScalingLazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(top = 32.dp, start = 8.dp, end = 8.dp, bottom = 32.dp),
        verticalArrangement = Arrangement.Center,
        state = listState
      ) {
        item {
          Text(
            modifier = contentModifier,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colors.primary,
            text = stringResource(R.string.app_name)
          )
        }
        item {
          Row(modifier = contentModifier, horizontalArrangement = Arrangement.SpaceEvenly) {
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.mute",
                    params = mapOf("mute" to true),
                  )
                )
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.VolumeOff,
                contentDescription = "mute volume",
                modifier = iconModifier
              )
            }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.volumeRelative",
                    params = mapOf("relativeSteps" to -1),
                  )
                )
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.VolumeDown,
                contentDescription = "turn volume down",
                modifier = iconModifier
              )
            }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.volumeRelative",
                    params = mapOf("relativeSteps" to 1),
                  )
                )
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.VolumeUp,
                contentDescription = "turn volume up",
                modifier = iconModifier
              )
            }
          }
        }
        item {
          Row(modifier = contentModifier, horizontalArrangement = Arrangement.SpaceEvenly) {
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.SetInput",
                    params = mapOf("newInput" to "1"),
                  )
                )
              },
            ) { Text("1") }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.SetInput",
                    params = mapOf("newInput" to "2"),
                  )
                )
              },
            ) { Text("2") }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.OnOff",
                    params = mapOf("on" to true),
                  )
                )
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.Power,
                contentDescription = "turn on tv",
                modifier = iconModifier
              )
            }
          }
        }
        item {
          Row(modifier = contentModifier, horizontalArrangement = Arrangement.SpaceEvenly) {
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.SetInput",
                    params = mapOf("newInput" to "3"),
                  )
                )
              },
            ) { Text("3") }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.SetInput",
                    params = mapOf("newInput" to "4"),
                  )
                )
              },
            ) { Text("4") }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.OnOff",
                    params = mapOf("on" to false),
                  )
                )
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.PowerOff,
                contentDescription = "turn off tv",
                modifier = iconModifier
              )
            }
          }
        }
        item {
          Chip(
            modifier = contentModifier,
            onClick = {
              sendPostRequest(
                Execution(
                  command = "action.devices.commands.wol",
                  params = null,
                )
              )
            },
            label = {
              Text(text = "Wake Serpens", maxLines = 1, overflow = TextOverflow.Ellipsis)
            },
            icon = {
              Icon(
                imageVector = Icons.Rounded.Transcribe,
                contentDescription = "wake on lan",
                modifier = iconModifier
              )
            },
          )
        }
      }
    }
  }
}

fun sendPostRequest(cmd: Execution) {
  val mURL = URL("https://cec.stevenandbonnie.com/fulfillment")
  CoroutineScope(Dispatchers.IO).launch {
    with(mURL.openConnection() as HttpURLConnection) {
      // optional default is GET
      requestMethod = "POST"
      setRequestProperty("Content-Type", "application/json")
      setRequestProperty(
        "Authorization",
        "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJjZWMuc3RldmVuYW5kYm9ubmllLmNvbSIsImV4cCI6NDc3ODQ1MjQ3MCwibmJmIjoxNjI0ODUyNDcwLCJpYXQiOjE2MjQ4NTI0NzB9.8DKqpmMDUMlkVMTPMgSgfXACDX1gQBs83K36vwHvzOg"
      )
      val req = FulfillmentRequest(
        requestId = "no-id",
        inputs = listOf(
          RequestPayload(
            intent = "action.devices.EXECUTE",
            payload = ExecutePayload(
              commands = listOf(
                ExecuteCommand(
                  devices = listOf(DeviceId(id = "123")),
                  execution = listOf(cmd),
                )
              ),
            )
          )
        ),
      )

      val wr = OutputStreamWriter(outputStream);
      val json = Gson().toJson(req)
      wr.write(json)
      wr.flush()

      Log.d("post", "content : $json")
      Log.d("post", "Response Code : $responseCode")

      BufferedReader(InputStreamReader(inputStream)).use {
        val response = StringBuffer()

        var inputLine = it.readLine()
        while (inputLine != null) {
          response.append(inputLine)
          inputLine = it.readLine()
        }
        Log.d("post", "Response : $response")
      }
    }
  }
}

data class FulfillmentRequest(val requestId: String, val inputs: List<RequestPayload>)
data class RequestPayload(val intent: String, val payload: ExecutePayload)
data class ExecutePayload(val commands: List<ExecuteCommand>)
data class ExecuteCommand(val devices: List<DeviceId>, val execution: List<Execution>)
data class DeviceId(val id: String)
data class Execution(val command: String, val params: Map<String, Any>?)

@Preview(device = Devices.WEAR_OS_SMALL_ROUND, showSystemUi = true)
@Composable
fun DefaultPreview() {
  WearApp()
}
