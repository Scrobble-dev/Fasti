package io.crates.keyring

import android.content.Context

class Keyring {
  companion object {
    init {
      System.loadLibrary("fasti_desktop")
    }

    external fun initializeNdkContext(context: Context)
  }
}
