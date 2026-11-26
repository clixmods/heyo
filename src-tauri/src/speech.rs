//! Mic button: speech-to-text inside the popup, with no OS panel.
//! Windows: the built-in WinRT speech recognizer (Windows.Media.SpeechRecognition), continuous
//! dictation. Partial results are sent to the popup as `dictation-partial`, final phrases as
//! `dictation-text`, and the end of the session as `dictation-stopped`.
//! Other systems: not available yet (the mic button is hidden).

use tauri::AppHandle;

/// Whether this OS can dictate from the popup.
pub const AVAILABLE: bool = cfg!(target_os = "windows");

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::Mutex;
    use tauri::{AppHandle, Emitter};
    use windows::Foundation::TypedEventHandler;
    use windows::Media::SpeechRecognition::{
        ISpeechRecognitionConstraint, SpeechContinuousRecognitionCompletedEventArgs,
        SpeechContinuousRecognitionResultGeneratedEventArgs, SpeechContinuousRecognitionSession,
        SpeechRecognitionHypothesisGeneratedEventArgs, SpeechRecognitionResultStatus,
        SpeechRecognitionScenario, SpeechRecognitionTopicConstraint, SpeechRecognizer,
    };
    use windows::core::{HSTRING, Interface, Ref};

    /// The running recognizer, kept alive until `stop`.
    static ACTIVE: Mutex<Option<SpeechRecognizer>> = Mutex::new(None);

    /// Windows refuses dictation until "Online speech recognition" is allowed.
    const PRIVACY_POLICY_NOT_ACCEPTED: u32 = 0x8004_5509;

    fn explain(e: windows::core::Error) -> String {
        if e.code().0 as u32 == PRIVACY_POLICY_NOT_ACCEPTED {
            "Dictation is off in Windows. Turn on Settings → Privacy & security → Speech → Online speech recognition, then try again.".into()
        } else {
            format!("Couldn't start the microphone: {}", e.message())
        }
    }

    pub fn start(app: &AppHandle) -> Result<(), String> {
        stop();
        let recognizer = SpeechRecognizer::new().map_err(explain)?;
        let dictation = SpeechRecognitionTopicConstraint::Create(
            SpeechRecognitionScenario::Dictation,
            &HSTRING::from("dictation"),
        )
        .map_err(explain)?;
        recognizer
            .Constraints()
            .and_then(|c| c.Append(&dictation.cast::<ISpeechRecognitionConstraint>()?))
            .map_err(explain)?;
        let compiled = recognizer
            .CompileConstraintsAsync()
            .and_then(|op| op.get())
            .map_err(explain)?;
        if compiled.Status().map_err(explain)? != SpeechRecognitionResultStatus::Success {
            return Err("Your Windows speech language doesn't support dictation. Check Settings → Time & language → Speech.".into());
        }

        let a = app.clone();
        recognizer
            .HypothesisGenerated(&TypedEventHandler::new(
                move |_, args: Ref<SpeechRecognitionHypothesisGeneratedEventArgs>| {
                    if let Ok(args) = args.ok() {
                        let text = args.Hypothesis()?.Text()?.to_string();
                        let _ = a.emit_to("popup", "dictation-partial", text);
                    }
                    Ok(())
                },
            ))
            .map_err(explain)?;

        let session = recognizer.ContinuousRecognitionSession().map_err(explain)?;
        let a = app.clone();
        session
            .ResultGenerated(&TypedEventHandler::new(
                move |_, args: Ref<SpeechContinuousRecognitionResultGeneratedEventArgs>| {
                    if let Ok(args) = args.ok() {
                        let result = args.Result()?;
                        if result.Status()? == SpeechRecognitionResultStatus::Success {
                            let text = result.Text()?.to_string();
                            if !text.trim().is_empty() {
                                let _ = a.emit_to("popup", "dictation-text", text);
                            }
                        }
                    }
                    Ok(())
                },
            ))
            .map_err(explain)?;
        let a = app.clone();
        session
            .Completed(&TypedEventHandler::new(
                move |_: Ref<SpeechContinuousRecognitionSession>,
                      _: Ref<SpeechContinuousRecognitionCompletedEventArgs>| {
                    let _ = a.emit_to("popup", "dictation-stopped", ());
                    Ok(())
                },
            ))
            .map_err(explain)?;

        session
            .StartAsync()
            .and_then(|op| op.get())
            .map_err(explain)?;
        *ACTIVE.lock().unwrap() = Some(recognizer);
        Ok(())
    }

    pub fn stop() {
        let Some(recognizer) = ACTIVE.lock().unwrap().take() else {
            return;
        };
        if let Ok(session) = recognizer.ContinuousRecognitionSession() {
            let _ = session.StopAsync().and_then(|op| op.get());
        }
        let _ = recognizer.Close();
    }
}

/// Starts listening. Blocking (WinRT async calls are awaited synchronously): call off the UI thread.
pub fn start(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        imp::start(app)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Dictation is only available on Windows for now.".into())
    }
}

/// Stops listening, if it was. Safe to call any time.
pub fn stop() {
    #[cfg(target_os = "windows")]
    imp::stop();
}
