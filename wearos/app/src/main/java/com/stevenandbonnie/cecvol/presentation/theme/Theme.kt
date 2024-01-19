package com.stevenandbonnie.cecvol.presentation.theme

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.wear.compose.material.Colors
import androidx.wear.compose.material.MaterialTheme

val Purple200 = Color(0xFFBB86FC)
val Purple700 = Color(0xFF3700B3)
val Teal200 = Color(0xFF03DAC5)
val Red400 = Color(0xFFCF6679)

@Composable
fun CECVolTheme(
    content: @Composable () -> Unit
) {
    /**
     * Empty theme to customize for your app.
     * See: https://developer.android.com/jetpack/compose/designsystems/custom
     */
    MaterialTheme(
        colors = Colors(
            primary = Purple200,
            primaryVariant = Purple700,
            secondary = Teal200,
            secondaryVariant = Teal200,
            error = Red400,
            onPrimary = Color.Black,
            onSecondary = Color.Black,
            onError = Color.Black
        ),
        content = content
    )
}