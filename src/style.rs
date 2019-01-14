use imgui::*;

// dark theme from codz01 (https://github.com/ocornut/imgui/issues/707)
pub fn set_style_dark(style: &mut ImGuiStyle) {
    style.frame_border_size = 1.0;
    style.frame_padding = ImVec2::new(4.0,2.0);
    style.item_spacing = ImVec2::new(8.0,2.0);
    style.window_border_size = 1.0;
    //style.tab_border_size = 1.0;
    style.window_rounding = 1.0;
    style.child_rounding = 1.0;
    style.frame_rounding = 1.0;
    style.scrollbar_rounding = 1.0;
    style.grab_rounding = 1.0;

    style.colors =
        [
        ImVec4::new(1.00, 1.00, 1.00, 0.95), // ImGuiCol_Text 
        ImVec4::new(0.50, 0.50, 0.50, 1.00), // ImGuiCol_TextDisabled 
        ImVec4::new(0.13, 0.12, 0.12, 1.00), // ImGuiCol_WindowBg 
        ImVec4::new(1.00, 1.00, 1.00, 0.00), // ImGuiCol_ChildBg 
        ImVec4::new(0.05, 0.05, 0.05, 0.94), // ImGuiCol_PopupBg 
        ImVec4::new(0.53, 0.53, 0.53, 0.46), // ImGuiCol_Border 
        ImVec4::new(0.00, 0.00, 0.00, 0.00), // ImGuiCol_BorderShadow 
        ImVec4::new(0.00, 0.00, 0.00, 0.85), // ImGuiCol_FrameBg 
        ImVec4::new(0.22, 0.22, 0.22, 0.40), // ImGuiCol_FrameBgHovered 
        ImVec4::new(0.16, 0.16, 0.16, 0.53), // ImGuiCol_FrameBgActive 
        ImVec4::new(0.00, 0.00, 0.00, 1.00), // ImGuiCol_TitleBg 
        ImVec4::new(0.00, 0.00, 0.00, 1.00), // ImGuiCol_TitleBgActive 
        ImVec4::new(0.00, 0.00, 0.00, 0.51), // ImGuiCol_TitleBgCollapsed 
        ImVec4::new(0.12, 0.12, 0.12, 1.00), // ImGuiCol_MenuBarBg 
        ImVec4::new(0.02, 0.02, 0.02, 0.53), // ImGuiCol_ScrollbarBg 
        ImVec4::new(0.31, 0.31, 0.31, 1.00), // ImGuiCol_ScrollbarGrab 
        ImVec4::new(0.41, 0.41, 0.41, 1.00), // ImGuiCol_ScrollbarGrabHovered 
        ImVec4::new(0.48, 0.48, 0.48, 1.00), // ImGuiCol_ScrollbarGrabActive 
        ImVec4::new(0.79, 0.79, 0.79, 1.00), // ImGuiCol_CheckMark 
        ImVec4::new(0.48, 0.47, 0.47, 0.91), // ImGuiCol_SliderGrab 
        ImVec4::new(0.56, 0.55, 0.55, 0.62), // ImGuiCol_SliderGrabActive 
        ImVec4::new(0.50, 0.50, 0.50, 0.63), // ImGuiCol_Button 
        ImVec4::new(0.67, 0.67, 0.68, 0.63), // ImGuiCol_ButtonHovered 
        ImVec4::new(0.26, 0.26, 0.26, 0.63), // ImGuiCol_ButtonActive 
        ImVec4::new(0.54, 0.54, 0.54, 0.58), // ImGuiCol_Header 
        ImVec4::new(0.64, 0.65, 0.65, 0.80), // ImGuiCol_HeaderHovered 
        ImVec4::new(0.25, 0.25, 0.25, 0.80), // ImGuiCol_HeaderActive 
        ImVec4::new(0.58, 0.58, 0.58, 0.50), // ImGuiCol_Separator 
        ImVec4::new(0.81, 0.81, 0.81, 0.64), // ImGuiCol_SeparatorHovered 
        ImVec4::new(0.81, 0.81, 0.81, 0.64), // ImGuiCol_SeparatorActive 
        ImVec4::new(0.87, 0.87, 0.87, 0.53), // ImGuiCol_ResizeGrip 
        ImVec4::new(0.87, 0.87, 0.87, 0.74), // ImGuiCol_ResizeGripHovered 
        ImVec4::new(0.87, 0.87, 0.87, 0.74), // ImGuiCol_ResizeGripActive 
        ImVec4::new(0.61, 0.61, 0.61, 1.00), // ImGuiCol_PlotLines 
        ImVec4::new(0.68, 0.68, 0.68, 1.00), // ImGuiCol_PlotLinesHovered 
        ImVec4::new(0.90, 0.77, 0.33, 1.00), // ImGuiCol_PlotHistogram 
        ImVec4::new(0.87, 0.55, 0.08, 1.00), // ImGuiCol_PlotHistogramHovered 
        ImVec4::new(0.47, 0.60, 0.76, 0.47), // ImGuiCol_TextSelectedBg 
        ImVec4::new(0.58, 0.58, 0.58, 0.90), // ImGuiCol_DragDropTarget 
        ImVec4::new(0.60, 0.60, 0.60, 1.00), // ImGuiCol_NavHighlight 
        ImVec4::new(1.00, 1.00, 1.00, 0.70), // ImGuiCol_NavWindowingHighlight 
        ImVec4::new(0.80, 0.80, 0.80, 0.20), // ImGuiCol_NavWindowingDimBg 
        ImVec4::new(0.80, 0.80, 0.80, 0.35), // ImGuiCol_ModalWindowDimBg 
        ];
}

// light green from @ebachard (https://github.com/ocornut/imgui/issues/707)
pub fn set_style_light(style: &mut ImGuiStyle) {
    style.window_rounding     = 2.0;
    style.scrollbar_rounding  = 3.0;
    style.grab_rounding       = 2.0;
    style.anti_aliased_lines  = true;
    style.anti_aliased_fill   = true;
    style.window_rounding     = 2.0;
    style.child_rounding      = 2.0;
    style.scrollbar_size      = 16.0;
    style.scrollbar_rounding  = 3.0;
    style.grab_rounding       = 2.0;
    style.item_spacing.x      = 10.0;
    style.item_spacing.y      = 4.0;
    style.indent_spacing      = 22.0;
    style.frame_padding.x     = 6.0;
    style.frame_padding.y     = 4.0;
    style.alpha               = 1.0;
    style.frame_rounding      = 3.0;

    style.colors =
        [
        ImVec4::new(0.00, 0.00, 0.00, 1.00), // ImGuiCol_Text
        ImVec4::new(0.60, 0.60, 0.60, 1.00), // ImGuiCol_TextDisabled
        ImVec4::new(0.86, 0.86, 0.86, 1.00), // ImGuiCol_WindowBg
        ImVec4::new(0.00, 0.00, 0.00, 0.00), // ImGuiCol_ChildBg
        ImVec4::new(0.93, 0.93, 0.93, 0.98), // ImGuiCol_PopupBg
        ImVec4::new(0.71, 0.71, 0.71, 0.08), // ImGuiCol_Border
        ImVec4::new(0.00, 0.00, 0.00, 0.04), // ImGuiCol_BorderShadow
        ImVec4::new(0.71, 0.71, 0.71, 0.55), // ImGuiCol_FrameBg
        ImVec4::new(0.94, 0.94, 0.94, 0.55), // ImGuiCol_FrameBgHovered
        ImVec4::new(0.71, 0.78, 0.69, 0.98), // ImGuiCol_FrameBgActive
        ImVec4::new(0.85, 0.85, 0.85, 1.00), // ImGuiCol_TitleBg
        ImVec4::new(0.78, 0.78, 0.78, 1.00), // ImGuiCol_TitleBgActive
        ImVec4::new(0.82, 0.78, 0.78, 0.51), // ImGuiCol_TitleBgCollapsed
        ImVec4::new(0.86, 0.86, 0.86, 1.00), // ImGuiCol_MenuBarBg
        ImVec4::new(0.20, 0.25, 0.30, 0.61), // ImGuiCol_ScrollbarBg
        ImVec4::new(0.90, 0.90, 0.90, 0.30), // ImGuiCol_ScrollbarGrab
        ImVec4::new(0.92, 0.92, 0.92, 0.78), // ImGuiCol_ScrollbarGrabHovered
        ImVec4::new(1.00, 1.00, 1.00, 1.00), // ImGuiCol_ScrollbarGrabActive
        ImVec4::new(0.184, 0.407, 0.193, 1.00), // ImGuiCol_CheckMark
        ImVec4::new(0.26, 0.59, 0.98, 0.78), // ImGuiCol_SliderGrab
        ImVec4::new(0.26, 0.59, 0.98, 1.00), // ImGuiCol_SliderGrabActive
        ImVec4::new(0.71, 0.78, 0.69, 0.40), // ImGuiCol_Button
        ImVec4::new(0.725, 0.805, 0.702, 1.00), // ImGuiCol_ButtonHovered
        ImVec4::new(0.793, 0.900, 0.836, 1.00), // ImGuiCol_ButtonActive
        ImVec4::new(0.71, 0.78, 0.69, 0.31), // ImGuiCol_Header
        ImVec4::new(0.71, 0.78, 0.69, 0.80), // ImGuiCol_HeaderHovered
        ImVec4::new(0.71, 0.78, 0.69, 1.00), // ImGuiCol_HeaderActive
        ImVec4::new(0.39, 0.39, 0.39, 1.00), // ImGuiCol_Separator
        ImVec4::new(0.14, 0.44, 0.80, 0.78), // ImGuiCol_SeparatorHovered
        ImVec4::new(0.14, 0.44, 0.80, 1.00), // ImGuiCol_SeparatorActive
        ImVec4::new(1.00, 1.00, 1.00, 0.00), // ImGuiCol_ResizeGrip
        ImVec4::new(0.26, 0.59, 0.98, 0.45), // ImGuiCol_ResizeGripHovered
        ImVec4::new(0.26, 0.59, 0.98, 0.78), // ImGuiCol_ResizeGripActive
        ImVec4::new(0.39, 0.39, 0.39, 1.00), // ImGuiCol_PlotLines
        ImVec4::new(1.00, 0.43, 0.35, 1.00), // ImGuiCol_PlotLinesHovered
        ImVec4::new(0.90, 0.70, 0.00, 1.00), // ImGuiCol_PlotHistogram
        ImVec4::new(1.00, 0.60, 0.00, 1.00), // ImGuiCol_PlotHistogramHovered
        ImVec4::new(0.26, 0.59, 0.98, 0.35), // ImGuiCol_TextSelectedBg
        ImVec4::new(0.26, 0.59, 0.98, 0.95), // ImGuiCol_DragDropTarget
        ImVec4::new(0.71, 0.78, 0.69, 0.80), // ImGuiCol_NavHighlight 
        ImVec4::new(0.70, 0.70, 0.70, 0.70), // ImGuiCol_NavWindowingHighlight 
        ImVec4::new(0.70, 0.70, 0.70, 0.30), // ImGuiCol_NavWindowingHighlight 
        ImVec4::new(0.20, 0.20, 0.20, 0.35), // ImGuiCol_ModalWindowDarkening
        ];
}

