/* While this template provides a good starting point for using Wear Compose, you can always
 * take a look at https://github.com/android/wear-os-samples/tree/main/ComposeStarter and
 * https://github.com/android/wear-os-samples/tree/main/ComposeAdvanced to find the most up to date
 * changes to the libraries and their usages.
 */

package com.stevenandbonnie.cecvol.presentation

import android.app.Activity
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Base64
import android.util.Log
import android.view.inputmethod.EditorInfo
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Key
import androidx.compose.material.icons.rounded.Power
import androidx.compose.material.icons.rounded.PowerOff
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Transcribe
import androidx.compose.material.icons.rounded.VolumeDown
import androidx.compose.material.icons.rounded.VolumeOff
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import androidx.wear.compose.foundation.ExperimentalWearFoundationApi
import androidx.wear.compose.foundation.lazy.ScalingLazyColumn
import androidx.wear.compose.foundation.lazy.rememberScalingLazyListState
import androidx.wear.compose.foundation.rememberActiveFocusRequester
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.ButtonDefaults
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.Icon
import androidx.wear.compose.material.MaterialTheme
import androidx.wear.compose.material.Scaffold
import androidx.wear.compose.material.Text
import androidx.wear.compose.material.TimeText
import androidx.wear.compose.material.Vignette
import androidx.wear.compose.material.VignettePosition
import androidx.wear.compose.material.scrollAway
import androidx.wear.input.RemoteInputIntentHelper
import androidx.wear.input.wearableExtender
import androidx.wear.tooling.preview.devices.WearDevices
import com.google.android.horologist.annotations.ExperimentalHorologistApi
import com.google.android.horologist.compose.rotaryinput.rotaryWithScroll
import com.google.gson.Gson
import com.stevenandbonnie.cecvol.R
import com.stevenandbonnie.cecvol.presentation.theme.CECVolTheme
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.single
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import java.io.IOException

val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "settings")

class MainActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    installSplashScreen()

    super.onCreate(savedInstanceState)

    setTheme(android.R.style.Theme_DeviceDefault)

    setContent { WearApp() }
  }
}

@OptIn(ExperimentalHorologistApi::class, ExperimentalWearFoundationApi::class)
@Composable
fun WearApp() {
  CECVolTheme {
    val context = LocalContext.current
    val listState = rememberScalingLazyListState()
    Scaffold(
      timeText = { TimeText(modifier = Modifier.scrollAway(listState)) },
      vignette = { Vignette(vignettePosition = VignettePosition.TopAndBottom) },
    ) {
      val contentModifier = Modifier
        .fillMaxWidth()
        .padding(bottom = 8.dp)
      val iconModifier = Modifier
        .size(32.dp)
        .wrapContentSize(align = Alignment.Center)
      val focusRequester = rememberActiveFocusRequester()
      ScalingLazyColumn(
        modifier = Modifier.rotaryWithScroll(
          scrollableState = listState,
          focusRequester = focusRequester,
        ), verticalArrangement = Arrangement.Center, state = listState
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
                    command = "action.devices.commands.OnOff",
                    params = mapOf("on" to false),
                  ), context
                )
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.PowerOff,
                contentDescription = "turn off tv",
                modifier = iconModifier
              )
            }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                sendPostRequest(
                  Execution(
                    command = "action.devices.commands.OnOff",
                    params = mapOf("on" to true),
                  ), context
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
                    command = "action.devices.commands.mute",
                    params = mapOf("mute" to true),
                  ), context
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
                  ), context
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
                  ), context
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
                  ), context
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
                  ), context
                )
              },
            ) { Text("2") }
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
                  ), context
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
                  ), context
                )
              },
            ) { Text("4") }
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
                ), context
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
        item {
          val websiteLauncher = rememberLauncherForActivityResult(
            contract = ActivityResultContracts.StartActivityForResult()
          ) { result ->
            if (result.resultCode == Activity.RESULT_OK) {
              CoroutineScope(Dispatchers.IO).launch {
                val name = RemoteInput.getResultsFromIntent(result.data).getString(WEBSITE_URL)
                name?.let { context.dataStore.edit { s -> s[WEBSITE_URL_PREF] = name } }
              }
            }
          }
          val userPassLauncher = rememberLauncherForActivityResult(
            contract = ActivityResultContracts.StartActivityForResult()
          ) { result ->
            if (result.resultCode == Activity.RESULT_OK) {
              CoroutineScope(Dispatchers.IO).launch {
                val name = RemoteInput.getResultsFromIntent(result.data).getString(WEBSITE_USERNAME)
                name?.let { context.dataStore.edit { s -> s[WEBSITE_USERNAME_PREF] = name } }
                val pass = RemoteInput.getResultsFromIntent(result.data).getString(WEBSITE_PASSWORD)
                pass?.let { context.dataStore.edit { s -> s[WEBSITE_PASSWORD_PREF] = pass } }
              }
            }
          }
          Row(modifier = contentModifier, horizontalArrangement = Arrangement.SpaceEvenly) {
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                val intent: Intent = RemoteInputIntentHelper.createActionRemoteInputIntent()
                val remoteInputs: List<RemoteInput> = listOf(
                  RemoteInput.Builder(WEBSITE_URL).setLabel("Website URL").wearableExtender {
                      setEmojisAllowed(false)
                      setInputActionType(EditorInfo.IME_ACTION_DONE)
                    }.build(),
                )
                RemoteInputIntentHelper.putRemoteInputsExtra(intent, remoteInputs)
                websiteLauncher.launch(intent)
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.Public,
                contentDescription = "website",
                modifier = iconModifier
              )
            }
            Button(
              modifier = Modifier.size(ButtonDefaults.SmallButtonSize),
              onClick = {
                val intent: Intent = RemoteInputIntentHelper.createActionRemoteInputIntent()
                val remoteInputs: List<RemoteInput> =
                  listOf(RemoteInput.Builder(WEBSITE_USERNAME).setLabel("Username")
                    .wearableExtender {
                      setEmojisAllowed(false)
                      setInputActionType(EditorInfo.IME_ACTION_DONE)
                    }.build(),
                    RemoteInput.Builder(WEBSITE_PASSWORD).setLabel("Password").wearableExtender {
                        setEmojisAllowed(false)
                        setInputActionType(EditorInfo.IME_ACTION_DONE)
                      }.build())
                RemoteInputIntentHelper.putRemoteInputsExtra(intent, remoteInputs)
                userPassLauncher.launch(intent)
              },
            ) {
              Icon(
                imageVector = Icons.Rounded.Key,
                contentDescription = "login info",
                modifier = iconModifier
              )
            }
          }
        }
      }
    }
  }
}

fun sendPostRequest(cmd: Execution, context: Context) {
  CoroutineScope(Dispatchers.IO).launch {
    val settings = context.dataStore.data.first()
    val mURL = URL("https://" + settings[WEBSITE_URL_PREF] + "/fulfillment")
    val auth = settings[WEBSITE_USERNAME_PREF] + ":" + settings[WEBSITE_PASSWORD_PREF]
    with(mURL.openConnection() as HttpURLConnection) {
      requestMethod = "POST"
      setRequestProperty("Content-Type", "application/json")
      setRequestProperty(
        "Authorization", "Basic " + Base64.encodeToString(auth.toByteArray(), Base64.DEFAULT)
      )
      val req = FulfillmentRequest(
        requestId = "no-id",
        inputs = listOf(
          RequestPayload(
            intent = "action.devices.EXECUTE", payload = ExecutePayload(
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

      val wr = OutputStreamWriter(outputStream)
      val json = Gson().toJson(req)
      wr.write(json)
      wr.flush()

      Log.d("post", "content : $json")
      Log.d("post", "Response Code : $responseCode")

      if (responseCode == 200) {
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
}

data class FulfillmentRequest(val requestId: String, val inputs: List<RequestPayload>)

data class RequestPayload(val intent: String, val payload: ExecutePayload)

data class ExecutePayload(val commands: List<ExecuteCommand>)

data class ExecuteCommand(val devices: List<DeviceId>, val execution: List<Execution>)

data class DeviceId(val id: String)

data class Execution(val command: String, val params: Map<String, Any>?)

@Preview(device = WearDevices.SMALL_ROUND, showSystemUi = true)
@Composable
fun DefaultPreview() {
  WearApp()
}

private const val WEBSITE_URL = "website_url"
private val WEBSITE_URL_PREF = stringPreferencesKey(WEBSITE_URL)
private const val WEBSITE_USERNAME = "website_username"
private val WEBSITE_USERNAME_PREF = stringPreferencesKey(WEBSITE_USERNAME)
private const val WEBSITE_PASSWORD = "website_password"
private val WEBSITE_PASSWORD_PREF = stringPreferencesKey(WEBSITE_PASSWORD)
