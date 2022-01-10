use crate::gpio::OutputPin;
use embedded_hal::PwmPin;
use esp_idf_sys::*;

type Duty = u8;

const HPOINT: u32 = 0;

pub mod config {
    use crate::units::*;
    use super::*;

    pub struct TimerConfig {
        pub frequency: Hertz,
        pub speed_mode: ledc_mode_t,
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

pub trait HwTimer {
    fn timer() -> ledc_timer_t;
}

pub trait HwChannel {
    fn channel() -> ledc_channel_t;
}

pub struct Timer<T: HwTimer> {
    instance: T,
    speed_mode: ledc_mode_t,
}

impl<T: HwTimer> Timer<T> {
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

    pub fn release(self) -> Result<T, EspError> {
        Ok(self.instance)
    }
}

pub struct Channel<'a, C: HwChannel, T: HwTimer, P: OutputPin> {
    instance: C,
    timer: &'a Timer<T>,
    pin: P,
    duty: Duty,
}

// FIXME: Stop channel upon dropping.
impl<'a, C: HwChannel, T: HwTimer, P: OutputPin> Channel<'a, C, T, P> {
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

        // FIXME: Why is this nescessary? How can we do this exactly once? How
        // to release it once there is not active channel?
        esp!(unsafe { ledc_fade_func_install(0) })?;

        // SAFETY: As log as we have borrowed the timer, we are safe to use it.
        esp!(unsafe { ledc_channel_config(&channel_config) })?;

        Ok(Channel {
            instance,
            timer,
            pin,
            duty,
        })
    }

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

    fn disable(&mut self) {
        if self.update_duty(0).is_err() {
            panic!("disabling PWM failed!");
        }
    }

    fn enable(&mut self) {
        if self.update_duty(self.duty).is_err() {
            panic!("enabling PWM failed!");
        }
    }

    fn get_duty(&self) -> Self::Duty {
        self.duty
    }

    fn get_max_duty(&self) -> Self::Duty {
        Duty::MAX
    }

    fn set_duty(&mut self, duty: Duty) {
        self.duty = duty;
        if self.update_duty(duty).is_err() {
            panic!("updating PWM failed!");
        }
    }
}

macro_rules! impl_timer {
    ($instance:ident: $timer:expr) => {
        pub struct $instance;

        impl $instance {
            pub unsafe fn new() -> Self {
                $instance {}
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
// FIMXE: Complete LEDC timer instances.

macro_rules! impl_channel {
    ($instance:ident: $channel:expr) => {
        pub struct $instance;

        impl $instance {
            pub unsafe fn new() -> Self {
                $instance {}
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
// FIXME: Complete LEDC channel instances.
