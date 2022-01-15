//! LED Control peripheral (which also crates PWM signals for other purposes)
//!
//! Interface to the [LED Control (LEDC)
//! peripheral](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/ledc.html)
//!
//! # Example
//!
//! Create a 25 kHz PWM signal with 25 % duty cycle on GPIO 1
//! ```
//! use esp_idf_hal::ledc::{
//!     config::TimerConfig,
//!     Channel,
//!     Timer,
//! };
//! use esp_idf_hal::peripherals::Peripherals;
//! use esp_idf_hal::prelude::*;
//!
//! let peripherals = Peripherals::take().unwrap();
//! let config = TimerConfig::default().frequency(25.kHz().into());
//! let timer = Timer::new(peripherals.ledc.timer0, &config)?;
//! let channel = Channel::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio1)?;
//!
//! channel.set_duty(64);
//! ```

use crate::gpio::OutputPin;
use embedded_hal::pwm::blocking::PwmPin;
use esp_idf_sys::*;
use lazy_static::lazy_static;
use std::sync::Mutex;

pub use chip::*;

type Duty = u8;

const HPOINT: u32 = 0;

lazy_static! {
    static ref FADE_FUNC_INSTALLED: Mutex<bool> = Mutex::new(false);
}

/// Types for configuring the LED Control peripheral
pub mod config {
    use crate::units::*;
    use super::*;

    pub struct TimerConfig {
        pub frequency: Hertz,
        pub speed_mode: ledc_mode_t,
    }

    impl TimerConfig {
        pub fn frequency(mut self, f: Hertz) -> Self {
            self.frequency = f;
            self
        }

        pub fn speed_mode(mut self, mode: ledc_mode_t) -> Self {
            self.speed_mode = mode;
            self
        }
    }

    impl Default for TimerConfig {
        fn default() -> Self {
            TimerConfig {
                frequency: 1000.Hz(),
                speed_mode: ledc_mode_t_LEDC_LOW_SPEED_MODE,
            }
        }
    }
}

/// LED Control timer abstraction
pub struct Timer<T: HwTimer> {
    instance: T,
    speed_mode: ledc_mode_t,
}

impl<T: HwTimer> Timer<T> {
    /// Creates a new LED Control timer abstraction
    pub fn new(instance: T, config: &config::TimerConfig) -> Result<Self, EspError> {
        let timer_config = ledc_timer_config_t {
            speed_mode: config.speed_mode,
            timer_num: T::timer(),
            __bindgen_anon_1: ledc_timer_config_t__bindgen_ty_1 { duty_resolution: ledc_timer_bit_t_LEDC_TIMER_8_BIT },
            freq_hz: config.frequency.into(),
            clk_cfg: ledc_clk_cfg_t_LEDC_AUTO_CLK,
        };

        // SAFETY: We own the instance and therefor are safe to configure it.
        esp!(unsafe { ledc_timer_config(&timer_config) })?;

        Ok(Timer {
            instance,
            speed_mode: config.speed_mode,
        })
    }

    /// Releases the timer peripheral
    pub fn release(self) -> Result<T, EspError> {
        Ok(self.instance)
    }
}

/// LED Control output channel abstraction
pub struct Channel<'a, C: HwChannel, T: HwTimer, P: OutputPin> {
    instance: C,
    timer: &'a Timer<T>,
    pin: P,
    duty: Duty,
}

// FIXME: Stop channel upon dropping.
impl<'a, C: HwChannel, T: HwTimer, P: OutputPin> Channel<'a, C, T, P> {
    /// Creates a new LED Control output channel abstraction
    pub fn new(instance: C, timer: &'a Timer<T>, pin: P) -> Result<Self, EspError> {
        let duty = 0u8;
        let channel_config = ledc_channel_config_t {
            speed_mode: timer.speed_mode,
            channel: C::channel(),
            timer_sel: T::timer(),
            intr_type: ledc_intr_type_t_LEDC_INTR_DISABLE,
            gpio_num: pin.pin(),
            duty: duty as u32,
            // TODO: Cross-check why hpoint is a i32 here and an u32 at
            // ledc_set_duty_and_update.
            hpoint: HPOINT as i32,
        };

        let mut installed = FADE_FUNC_INSTALLED.lock().unwrap();
        if !*installed {
            // FIXME: Why is this nescessary? How to release it once there is
            // not active channel?
            esp!(unsafe { ledc_fade_func_install(0) })?;
            *installed = true;
        }
        drop(installed);

        // SAFETY: As long as we have borrowed the timer, we are safe to use
        // it.
        esp!(unsafe { ledc_channel_config(&channel_config) })?;

        Ok(Channel {
            instance,
            timer,
            pin,
            duty,
        })
    }

    /// Releases the output channel peripheral
    pub fn release(self) -> Result<(C, P), EspError> {
        Ok((self.instance, self.pin))
    }

    fn update_duty(&mut self, duty: Duty) -> Result<(), EspError> {
        esp!(unsafe { ledc_set_duty_and_update(self.timer.speed_mode, C::channel(), duty as u32, HPOINT) })?;
        Ok(())
    }
}

impl<'a, C: HwChannel, T:HwTimer, P: OutputPin>  PwmPin for Channel<'a, C, T, P> {
    type Duty = Duty;
    type Error = EspError;

    fn disable(&mut self) -> Result<(), Self::Error> {
        self.update_duty(0)?;
        Ok(())
    }

    fn enable(&mut self) -> Result<(), Self::Error> {
        self.update_duty(self.duty)?;
        Ok(())
    }

    fn get_duty(&self) -> Result::<Self::Duty, Self::Error> {
        Ok(self.duty)
    }

    fn get_max_duty(&self) -> Result::<Self::Duty, Self::Error> {
        Ok(Duty::MAX)
    }

    fn set_duty(&mut self, duty: Duty) -> Result<(), Self::Error> {
        self.duty = duty;
        self.update_duty(duty)?;
        Ok(())
    }
}

mod chip {
    use core::marker::PhantomData;
    use esp_idf_sys::*;

    /// LED Control peripheral timer
    pub trait HwTimer {
        fn timer() -> ledc_timer_t;
    }

    /// LED Control peripheral output channel
    pub trait HwChannel {
        fn channel() -> ledc_channel_t;
    }

    macro_rules! impl_timer {
        ($instance:ident: $timer:expr) => {
            pub struct $instance {
                _marker: PhantomData<*const ()>,
            }

            impl $instance {
                pub unsafe fn new() -> Self {
                    $instance { _marker: PhantomData }
                }
            }

            impl HwTimer for $instance {
                fn timer() -> ledc_timer_t {
                    $timer
                }
            }
        }
    }

    impl_timer!(TIMER0: ledc_timer_t_LEDC_TIMER_0);
    impl_timer!(TIMER1: ledc_timer_t_LEDC_TIMER_1);
    impl_timer!(TIMER2: ledc_timer_t_LEDC_TIMER_2);
    impl_timer!(TIMER3: ledc_timer_t_LEDC_TIMER_3);

    macro_rules! impl_channel {
        ($instance:ident: $channel:expr) => {
            pub struct $instance {
                _marker: PhantomData<*const ()>,
            }

            impl $instance {
                pub unsafe fn new() -> Self {
                    $instance { _marker: PhantomData }
                }
            }

            impl HwChannel for $instance {
                fn channel() -> ledc_channel_t {
                    $channel
                }
            }
        }
    }

    impl_channel!(CHANNEL0: ledc_channel_t_LEDC_CHANNEL_0);
    impl_channel!(CHANNEL1: ledc_channel_t_LEDC_CHANNEL_1);
    impl_channel!(CHANNEL2: ledc_channel_t_LEDC_CHANNEL_2);
    impl_channel!(CHANNEL3: ledc_channel_t_LEDC_CHANNEL_3);
    impl_channel!(CHANNEL4: ledc_channel_t_LEDC_CHANNEL_4);
    impl_channel!(CHANNEL5: ledc_channel_t_LEDC_CHANNEL_5);

    /// The LED Control device peripheral
    pub struct Peripheral {
        pub timer0: TIMER0,
        pub timer1: TIMER1,
        pub timer2: TIMER2,
        pub timer3: TIMER3,
        pub channel0: CHANNEL0,
        pub channel1: CHANNEL1,
        pub channel2: CHANNEL2,
        pub channel3: CHANNEL3,
        pub channel4: CHANNEL4,
        pub channel5: CHANNEL5,
        #[cfg(any(esp32, esp32s2, esp32s3, esp8684))]
        pub channel6: CHANNEL6,
        #[cfg(any(esp32, esp32s2, esp32s3, esp8684))]
        pub channel7: CHANNEL7,
    }

    impl Peripheral {
        /// Creates a new instance of the LEDC peripheral. Typically one wants
        /// to use the instance [`ledc`](crate::peripherals::Peripherals::ledc) from
        /// the device peripherals obtained via
        /// [`peripherals::Peripherals::take()`](crate::peripherals::Peripherals::take()).
        ///
        /// # Safety
        ///
        /// It is safe to instantiate the LEDC peripheral exactly one time.
        /// Care has to be taken that this has not already been done elsewhere.
        pub unsafe fn new() -> Self {
            Self {
                timer0: TIMER0::new(),
                timer1: TIMER1::new(),
                timer2: TIMER2::new(),
                timer3: TIMER3::new(),
                channel0: CHANNEL0::new(),
                channel1: CHANNEL1::new(),
                channel2: CHANNEL2::new(),
                channel3: CHANNEL3::new(),
                channel4: CHANNEL4::new(),
                channel5: CHANNEL5::new(),
                #[cfg(any(esp32, esp32s2, esp32s3, esp8684))]
                channel6: CHANNEL6::new(),
                #[cfg(any(esp32, esp32s2, esp32s3, esp8684))]
                channel7: CHANNEL7::new(),
            }
        }
    }
}
