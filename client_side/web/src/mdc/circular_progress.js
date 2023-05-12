//See: https://github.com/material-components/material-components-web/blob/master/docs/integrating-into-frameworks.md#the-simple-approach-wrapping-mdc-web-vanilla-components
//Also: https://material-components.github.io/material-components-web/classes/_mdc_textfield_component_.mdctextfield.html#constructor
export function register_circular_progress(node) {
    console.log("Creating circle", node);
    //Also works? mdc.textField.MDCTextField.attachTo(node);
    let result = new mdc.circularProgress.MDCCircularProgress(node);
    console.log(result);
    //result.progress = 0.25;
    result.determinate = false;
    return result
}

export function set_determinate(mdc_circular_progress, is_determinate) {
    mdc_circular_progress.determinate = is_determinate
}

export function set_progress(mdc_circular_progress, progress) {
    mdc_circular_progress.progress = progress;
}

export function deregister_circular_progress(mdc_circular_progress) {
    console.log("Destroying circle!")
    mdc_circular_progress.destroy();
}