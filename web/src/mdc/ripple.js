//See: https://github.com/material-components/material-components-web/blob/master/docs/integrating-into-frameworks.md#the-simple-approach-wrapping-mdc-web-vanilla-components
//Also: https://material-components.github.io/material-components-web/classes/_mdc_textfield_component_.mdctextfield.html#constructor
export function register_ripple(node) {
    console.log("Creating ripple", node);
    let ripple = new mdc.ripple.MDCRipple(node);
    ripple.unbounded = true; // Does it work without this? No. Dunno why.
    return ripple;
}

export function deregister_ripple(mdc_ripple) {
    console.log("Destroying ripple!")
    mdc_ripple.destroy();
}