use esp_idf_sys::*;

pub struct Timer {
}

impl Timer {
    pub fn foo() -> Result<(), EspError> {
        let speed = ledc_mode_t_LEDC_LOW_SPEED_MODE;
        let timer = ledc_timer_t_LEDC_TIMER_0;

        let timer_config = ledc_timer_config_t {
            speed_mode: speed,
            timer_num: timer,
            // duty_resolution: ledc_timer_bit_t_LEDC_TIMER_8_BIT,
            __bindgen_anon_1: ledc_timer_config_t__bindgen_ty_1 { duty_resolution: ledc_timer_bit_t_LEDC_TIMER_8_BIT },
            freq_hz: 1000,
            clk_cfg: ledc_clk_cfg_t_LEDC_AUTO_CLK,
        };

        esp!(unsafe { ledc_timer_config(&timer_config) })?;

        let channel_config = ledc_channel_config_t {
            speed_mode: speed,
            channel: ledc_channel_t_LEDC_CHANNEL_0,
            timer_sel: timer,
            intr_type: ledc_intr_type_t_LEDC_INTR_DISABLE,
            gpio_num: 4,
            duty: 64,
            hpoint: 0,
        };

        esp!(unsafe { ledc_channel_config(&channel_config) })?;

        Ok(())
    }
}
