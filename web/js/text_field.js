//See: https://github.com/material-components/material-components-web/blob/master/docs/integrating-into-frameworks.md#the-simple-approach-wrapping-mdc-web-vanilla-components
//Also: https://material-components.github.io/material-components-web/classes/_mdc_textfield_component_.mdctextfield.html#constructor
export function register_text_field(node) {
    console.log("Creating text_field", node);
    //Also works? mdc.textField.MDCTextField.attachTo(node);
    return new mdc.textField.MDCTextField(node);
}

export function deregister_text_field(mdc_text_field) {
    console.log("Destroying!")
    mdc_text_field.destroy();
}